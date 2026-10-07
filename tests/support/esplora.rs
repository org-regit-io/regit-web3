// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Deterministic GET fixture for the concrete Esplora public API.

use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

pub(super) struct Reply {
    status: u16,
    body: String,
    delay: Duration,
}
impl Reply {
    pub(super) fn ok(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            body: body.into(),
            delay: Duration::ZERO,
        }
    }
    pub(super) fn status(status: u16, body: impl Into<String>) -> Self {
        Self {
            status,
            body: body.into(),
            delay: Duration::ZERO,
        }
    }
    pub(super) fn delayed(body: impl Into<String>, delay: Duration) -> Self {
        Self {
            status: 200,
            body: body.into(),
            delay,
        }
    }
}

pub(super) struct Fixture {
    endpoint: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<io::Result<()>>,
}
impl Fixture {
    pub(super) async fn start(replies: Vec<Reply>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/api", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            for reply in replies {
                let (mut socket, _) = listener.accept().await?;
                let mut bytes = Vec::new();
                let mut chunk = [0; 1024];
                while !bytes.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                    let count = socket.read(&mut chunk).await?;
                    if count == 0 || bytes.len() + count > 64 * 1024 {
                        return Err(io::Error::other("invalid fixture request"));
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                }
                captured
                    .lock()
                    .map_err(|_| io::Error::other("fixture lock"))?
                    .push(
                        String::from_utf8(bytes)
                            .map_err(|_| io::Error::other("fixture encoding"))?,
                    );
                let headers = format!(
                    "HTTP/1.1 {} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    reply.status,
                    reply.body.len()
                );
                socket.write_all(headers.as_bytes()).await?;
                tokio::time::sleep(reply.delay).await;
                socket.write_all(reply.body.as_bytes()).await?;
            }
            Ok(())
        });
        Ok(Self {
            endpoint,
            requests,
            task,
        })
    }
    pub(super) fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub(super) fn requests(&self) -> io::Result<Vec<String>> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .map_err(|_| io::Error::other("fixture lock"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
