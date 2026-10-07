// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Deterministic HTTP replies over an ephemeral loopback listener.

use std::{
    collections::VecDeque,
    fmt::Write as _,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::{JoinHandle, JoinSet},
};

#[derive(Clone, Debug)]
pub(super) struct Request {
    pub(super) target: String,
    pub(super) headers: String,
    pub(super) body: Value,
}

pub(super) struct Reply {
    pub(super) status: u16,
    pub(super) body: Vec<u8>,
    pub(super) echo_id: bool,
    pub(super) framing: Framing,
    pub(super) delay: Duration,
    pub(super) body_delay: Duration,
    pub(super) close_connection: bool,
    pub(super) headers: Vec<(String, String)>,
}

pub(super) enum Framing {
    ContentLength,
    CloseDelimited,
    Chunked,
}

impl Reply {
    pub(super) fn json(value: &Value) -> Result<Self, serde_json::Error> {
        Ok(Self {
            status: 200,
            body: serde_json::to_vec(value)?,
            echo_id: true,
            framing: Framing::ContentLength,
            delay: Duration::ZERO,
            body_delay: Duration::ZERO,
            close_connection: false,
            headers: Vec::new(),
        })
    }

    pub(super) fn result(value: &Value) -> Result<Self, serde_json::Error> {
        Self::json(&json!({"jsonrpc": "2.0", "id": null, "result": value}))
    }

    pub(super) fn raw(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            body,
            echo_id: false,
            framing: Framing::ContentLength,
            delay: Duration::ZERO,
            body_delay: Duration::ZERO,
            close_connection: false,
            headers: Vec::new(),
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
        let queued = Mutex::new(VecDeque::from(replies));
        Self::start_routed(move |_| {
            queued
                .lock()
                .map_err(|_| io::Error::other("fixture reply lock poisoned"))?
                .pop_front()
                .ok_or_else(|| io::Error::other("fixture received an unexpected request"))
        })
        .await
    }

    pub(super) async fn start_routed<F>(respond: F) -> io::Result<Self>
    where
        F: Fn(&Request) -> io::Result<Reply> + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/rpc", listener.local_addr()?);
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let respond = Arc::new(respond);
        let task = tokio::spawn(async move {
            let mut connections = JoinSet::new();
            loop {
                let (mut stream, _) = listener.accept().await?;
                let captured = Arc::clone(&captured);
                let respond = Arc::clone(&respond);
                connections.spawn(async move {
                    let request = read_request(&mut stream).await?;
                    captured
                        .lock()
                        .map_err(|_| io::Error::other("fixture request lock poisoned"))?
                        .push(request.clone());
                    let reply = respond(&request)?;
                    send_reply(&mut stream, reply, &request.body["id"]).await
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
            .map(|requests| requests.clone())
            .map_err(|_| io::Error::other("fixture request lock poisoned"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn read_request(stream: &mut TcpStream) -> io::Result<Request> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        if let Some(position) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break position + 4;
        }
        if bytes.len() > 32_768 {
            return Err(io::Error::other("fixture request headers too large"));
        }
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        bytes.extend_from_slice(&buffer[..read]);
    };
    let headers = std::str::from_utf8(&bytes[..header_end])
        .map_err(|_| io::Error::other("fixture request headers are not UTF-8"))?
        .to_owned();
    let target = headers
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .ok_or_else(|| io::Error::other("fixture request target missing"))?
        .to_owned();
    let length = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map_or(Ok(0), |(_, value)| {
            value
                .trim()
                .parse::<usize>()
                .map_err(|_| io::Error::other("fixture request length invalid"))
        })?;
    if length > 65_536 {
        return Err(io::Error::other("fixture request body too large"));
    }
    while bytes.len() < header_end + length {
        let read = stream.read(&mut buffer).await?;
        if read == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    let body = if length == 0 {
        Value::Null
    } else {
        serde_json::from_slice(&bytes[header_end..header_end + length])
            .map_err(|_| io::Error::other("fixture request body is not JSON"))?
    };
    Ok(Request {
        target,
        headers,
        body,
    })
}

async fn send_reply(stream: &mut TcpStream, mut reply: Reply, id: &Value) -> io::Result<()> {
    if reply.close_connection {
        return stream.shutdown().await;
    }
    if reply.echo_id {
        let mut value: Value = serde_json::from_slice(&reply.body)
            .map_err(|_| io::Error::other("fixture reply body is not JSON"))?;
        value["id"] = id.clone();
        reply.body = serde_json::to_vec(&value)
            .map_err(|_| io::Error::other("fixture reply serialization failed"))?;
    }
    tokio::time::sleep(reply.delay).await;
    let mut headers = format!(
        "HTTP/1.1 {} Fixture\r\nConnection: close\r\nContent-Type: application/json\r\n",
        reply.status
    );
    match reply.framing {
        Framing::Chunked => headers.push_str("Transfer-Encoding: chunked\r\n"),
        Framing::ContentLength => {
            write!(headers, "Content-Length: {}\r\n", reply.body.len())
                .map_err(|_| io::Error::other("fixture response headers formatting failed"))?;
        }
        Framing::CloseDelimited => {}
    }
    for (name, value) in reply.headers {
        write!(headers, "{name}: {value}\r\n")
            .map_err(|_| io::Error::other("fixture response headers formatting failed"))?;
    }
    headers.push_str("\r\n");
    stream.write_all(headers.as_bytes()).await?;
    tokio::time::sleep(reply.body_delay).await;
    if matches!(reply.framing, Framing::Chunked) {
        for chunk in reply.body.chunks(64) {
            stream
                .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                .await?;
            stream.write_all(chunk).await?;
            stream.write_all(b"\r\n").await?;
        }
        stream.write_all(b"0\r\n\r\n").await?;
    } else {
        stream.write_all(&reply.body).await?;
    }
    stream.shutdown().await
}
