// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Ephemeral read-only HTTP fixtures with captured targets and header credentials.
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
    pub(super) status: u16,
    pub(super) body: String,
    pub(super) delay: Duration,
}
impl Reply {
    pub(super) fn json(body: &str) -> Self {
        Self {
            status: 200,
            body: body.to_owned(),
            delay: Duration::ZERO,
        }
    }
    pub(super) fn status(status: u16) -> Self {
        Self {
            status,
            body: "private-provider-body".to_owned(),
            delay: Duration::ZERO,
        }
    }
}
pub(super) struct Fixture {
    pub(super) endpoint: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<io::Result<()>>,
}
impl Fixture {
    pub(super) async fn start(replies: Vec<Reply>) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/api/v3/", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            for reply in replies {
                let (mut stream, _) = listener.accept().await?;
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 1024];
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    if bytes.len() > 32768 {
                        return Err(io::Error::other("fixture header overflow"));
                    }
                    let count = stream.read(&mut buffer).await?;
                    if count == 0 {
                        return Err(io::ErrorKind::UnexpectedEof.into());
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                captured
                    .lock()
                    .map_err(|_| io::Error::other("fixture lock"))?
                    .push(
                        String::from_utf8(bytes)
                            .map_err(|_| io::Error::other("fixture header encoding"))?,
                    );
                stream.write_all(format!("HTTP/1.1 {} Fixture\r\ncontent-length: {}\r\nconnection: close\r\ncontent-type: application/json\r\n\r\n",reply.status,reply.body.len()).as_bytes()).await?;
                tokio::time::sleep(reply.delay).await;
                stream.write_all(reply.body.as_bytes()).await?;
                stream.shutdown().await?;
            }
            Ok(())
        });
        Ok(Self {
            endpoint,
            requests,
            task,
        })
    }
    pub(super) fn requests(&self) -> io::Result<Vec<String>> {
        self.requests
            .lock()
            .map(|v| v.clone())
            .map_err(|_| io::Error::other("fixture lock"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
