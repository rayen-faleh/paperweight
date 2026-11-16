//! The read-only calendar feed: `GET http://127.0.0.1:<port>/paperweight.ics`.
//!
//! A deliberately tiny HTTP/1.1 server on std only. It binds to loopback, so
//! nothing outside this computer can reach it. Each request opens the database
//! fresh, so the feed always reflects the latest data.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use chrono::Utc;

use crate::ics;
use crate::store::Store;

pub const FEED_PATH: &str = "/paperweight.ics";

pub fn feed_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}{FEED_PATH}")
}

pub struct FeedServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl FeedServer {
    /// Bind `127.0.0.1:port` (0 picks a free port) and serve in a thread.
    pub fn start(port: u16, db_path: PathBuf) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let handle = std::thread::Builder::new()
            .name("paperweight-feed".into())
            .spawn(move || {
                for conn in listener.incoming() {
                    if stop2.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = conn {
                        let path = db_path.clone();
                        let _ = std::thread::Builder::new()
                            .name("paperweight-feed-conn".into())
                            .spawn(move || handle_conn(stream, &path));
                    }
                }
            })?;
        Ok(FeedServer {
            addr,
            stop,
            handle: Some(handle),
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn url(&self) -> String {
        feed_url(self.addr.port())
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for FeedServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn respond(stream: &mut TcpStream, status: &str, ctype: &str, body: &[u8], head_only: bool) {
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    if !head_only {
        let _ = stream.write_all(body);
    }
    let _ = stream.flush();
}

fn handle_conn(mut stream: TcpStream, db_path: &std::path::Path) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    // Drain headers.
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let path = target.split('?').next().unwrap_or("");
    let head_only = method == "HEAD";
    if method != "GET" && !head_only {
        respond(
            &mut stream,
            "405 Method Not Allowed",
            "text/plain",
            b"Only GET is supported.\n",
            false,
        );
        return;
    }
    if path != FEED_PATH {
        respond(
            &mut stream,
            "404 Not Found",
            "text/plain",
            b"The feed is at /paperweight.ics\n",
            head_only,
        );
        return;
    }
    match render(db_path) {
        Ok(body) => respond(
            &mut stream,
            "200 OK",
            "text/calendar; charset=utf-8",
            body.as_bytes(),
            head_only,
        ),
        Err(e) => respond(
            &mut stream,
            "500 Internal Server Error",
            "text/plain",
            format!("Could not read the Paperweight database: {e}\n").as_bytes(),
            head_only,
        ),
    }
}

/// The feed body for the database at `db_path`.
pub fn render(db_path: &std::path::Path) -> crate::Result<String> {
    let store = Store::open(db_path)?;
    let clock = store.clock()?;
    let items = store.open_items()?;
    Ok(ics::calendar(&items, &clock, Utc::now()))
}
