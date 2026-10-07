// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

use std::{
    collections::VecDeque,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Request {
    pub(super) target: String,
    pub(super) headers: String,
    pub(super) body: Vec<u8>,
}
pub(super) struct Reply {
    pub(super) status: u16,
    pub(super) body: Vec<u8>,
    pub(super) header_delay: Duration,
    pub(super) body_delay: Duration,
}
impl Reply {
    pub(super) fn ok(body: impl AsRef<[u8]>) -> Self {
        Self {
            status: 200,
            body: body.as_ref().to_vec(),
            header_delay: Duration::ZERO,
            body_delay: Duration::ZERO,
        }
    }
    pub(super) fn status(status: u16, body: impl AsRef<[u8]>) -> Self {
        Self {
            status,
            ..Self::ok(body)
        }
    }
}
pub(super) struct Fixture {
    pub(super) endpoint: String,
    requests: Arc<Mutex<Vec<Request>>>,
    task: JoinHandle<io::Result<()>>,
}
impl Fixture {
    pub(super) async fn start(replies: Vec<Reply>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/v1", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let queued = Arc::new(Mutex::new(VecDeque::from(replies)));
        let task = tokio::spawn(async move {
            let mut jobs = JoinSet::new();
            loop {
                let (mut socket, _) = listener.accept().await?;
                let captured = Arc::clone(&captured);
                let queued = Arc::clone(&queued);
                jobs.spawn(async move{let request=read(&mut socket).await?;
                captured.lock().map_err(|_|io::Error::other("fixture capture lock"))?.push(request);
                let reply=queued.lock().map_err(|_|io::Error::other("fixture reply lock"))?.pop_front().ok_or_else(||io::Error::other("unexpected fixture request"))?;
                tokio::time::sleep(reply.header_delay).await;
                socket.write_all(format!("HTTP/1.1 {} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",reply.status,reply.body.len()).as_bytes()).await?;
                tokio::time::sleep(reply.body_delay).await;socket.write_all(&reply.body).await?;socket.shutdown().await
            });
            }
        });
        Ok(Self {
            endpoint,
            requests,
            task,
        })
    }
    pub(super) fn requests(&self) -> io::Result<Vec<Request>> {
        self.requests
            .lock()
            .map(|r| r.clone())
            .map_err(|_| io::Error::other("fixture capture lock"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
async fn read(socket: &mut TcpStream) -> io::Result<Request> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 4096];
    let end = loop {
        if let Some(p) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
            break p + 4;
        }
        if bytes.len() > 32768 {
            return Err(io::Error::other("fixture headers bound"));
        }
        let n = socket.read(&mut chunk).await?;
        if n == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        bytes.extend_from_slice(&chunk[..n]);
    };
    let headers = String::from_utf8(bytes[..end].to_vec())
        .map_err(|_| io::Error::other("fixture header encoding"))?;
    let target = headers
        .lines()
        .next()
        .and_then(|v| v.split_whitespace().nth(1))
        .ok_or_else(|| io::Error::other("fixture target"))?
        .to_owned();
    let length = headers
        .lines()
        .filter_map(|v| v.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map_or(Ok(0), |(_, v)| {
            v.trim()
                .parse::<usize>()
                .map_err(|_| io::Error::other("fixture request length"))
        })?;
    if length > 2 * 1024 * 1024 {
        return Err(io::Error::other("fixture request bound"));
    }
    while bytes.len() < end + length {
        let n = socket.read(&mut chunk).await?;
        if n == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    Ok(Request {
        target,
        headers,
        body: bytes[end..end + length].to_vec(),
    })
}
