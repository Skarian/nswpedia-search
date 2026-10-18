//! Tiny HTTP/1.1 server for tests, so no real Real-Debrid account is needed.
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
impl Request {
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone())
    }
}

pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}
impl Reply {
    pub fn json(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: body.as_bytes().to_vec(),
        }
    }
    pub fn bytes(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![],
            body,
        }
    }
    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

pub struct MockServer {
    port: u16,
    stop: Arc<AtomicBool>,
}
impl MockServer {
    pub fn start(handler: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let stop = Arc::new(AtomicBool::new(false));
        let handler = Arc::new(handler);
        let flag = stop.clone();
        thread::spawn(move || {
            for stream in listener.incoming() {
                if flag.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let handler = handler.clone();
                thread::spawn(move || serve(stream, &*handler));
            }
        });
        Self { port, stop }
    }
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}
impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn serve(stream: TcpStream, handler: &dyn Fn(&Request) -> Reply) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let path = parts.next().unwrap_or_default().to_string();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let mut request = Request {
        method,
        path,
        headers,
        body: vec![],
    };
    let length: usize = request
        .header("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    request.body = vec![0; length];
    let _ = reader.read_exact(&mut request.body);
    let reply = handler(&request);
    let mut out = stream;
    let mut head = format!(
        "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        reply.status,
        reply.body.len()
    );
    for (k, v) in &reply.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let _ = out.write_all(head.as_bytes());
    if request.method != "HEAD" {
        let _ = out.write_all(&reply.body);
    }
    let _ = out.flush();
}
