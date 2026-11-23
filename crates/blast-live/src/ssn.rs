use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::{Message, WebSocket};

use crate::twitch::{now_ms, tls_connection};
use crate::{ChatEvent, ChatMessage, Platform};

// [LIVE-20]
const IDLE: Duration = Duration::from_millis(15);
const PING_EVERY: Duration = Duration::from_secs(30);
const MAX_BACKOFF: u64 = 30;
const CHAT_IN: u8 = 4;
const COMMAND_OUT: u8 = 1;

// [LIVE-21]
#[derive(Debug, Clone, PartialEq)]
pub struct SsnConfig {
    pub session: String,
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl SsnConfig {
    pub fn new(session: &str) -> Self {
        Self {
            session: session_id(session),
            host: "io.socialstream.ninja".into(),
            port: 443,
            tls: true,
        }
    }

    fn url(&self) -> String {
        format!(
            "{}://{}:{}/join/{}/{CHAT_IN}/{COMMAND_OUT}",
            if self.tls { "wss" } else { "ws" },
            self.host,
            self.port,
            self.session
        )
    }
}

// [LIVE-22]
pub fn session_id(input: &str) -> String {
    let s = input.trim();
    let from_query = s
        .split(['?', '&'])
        .find_map(|p| p.strip_prefix("session=").or_else(|| p.strip_prefix("s=")));
    from_query
        .unwrap_or(s)
        .split(['&', '#', '/', ' '])
        .next()
        .unwrap_or("")
        .to_string()
}

// [LIVE-23]
enum Stream {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    fn sock(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Tls(t) => &t.sock,
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(t) => t.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(t) => t.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(t) => t.flush(),
        }
    }
}

// [LIVE-24]
fn open(cfg: &SsnConfig) -> Result<WebSocket<Stream>, String> {
    let addr = (cfg.host.as_str(), cfg.port)
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or("adresse introuvable")?;
    let sock =
        TcpStream::connect_timeout(&addr, Duration::from_secs(6)).map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_secs(8)))
        .map_err(|e| e.to_string())?;
    sock.set_nodelay(true).map_err(|e| e.to_string())?;
    let stream = if cfg.tls {
        let conn = tls_connection(&cfg.host).map_err(|e| e.to_string())?;
        Stream::Tls(Box::new(rustls::StreamOwned::new(conn, sock)))
    } else {
        Stream::Plain(sock)
    };
    let (ws, _) = tungstenite::client(cfg.url(), stream).map_err(|e| e.to_string())?;
    ws.get_ref()
        .sock()
        .set_nonblocking(true)
        .map_err(|e| e.to_string())?;
    Ok(ws)
}

// [LIVE-25]
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let tag = &rest[start..start + end + 1];
        if tag.to_ascii_lowercase().starts_with("<img") {
            if let Some(alt) = attr(tag, "alt") {
                out.push_str(&alt);
            }
        } else if tag.eq_ignore_ascii_case("<br>") || tag.eq_ignore_ascii_case("<br/>") {
            out.push(' ');
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    decode_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let low = tag.to_ascii_lowercase();
    let pos = low.find(&format!("{name}="))? + name.len() + 1;
    let rest = &tag[pos..];
    let quote = rest.chars().next()?;
    if quote == '"' || quote == '\'' {
        let end = rest[1..].find(quote)?;
        Some(decode_entities(&rest[1..1 + end]))
    } else {
        Some(rest.split([' ', '>', '/']).next()?.to_string())
    }
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(true) => "1".into(),
        _ => String::new(),
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty() && s != "false" && s != "0",
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::Null => false,
        _ => true,
    }
}

// [LIVE-26]
pub fn parse_message(json_text: &str) -> Option<ChatEvent> {
    let v: Value = serde_json::from_str(json_text).ok()?;
    let author = text_of(v.get("chatname")?);
    let raw = text_of(&v["chatmessage"]);
    let text = if truthy(&v["textonly"]) {
        raw.trim().to_string()
    } else {
        strip_html(&raw)
    };
    let source = text_of(&v["type"]).to_ascii_lowercase();
    if truthy(&v["event"]) {
        let text = if text.is_empty() {
            author
        } else {
            format!("{author} {text}")
        };
        return Some(ChatEvent::Notice { text });
    }
    let mut badges = Vec::new();
    if truthy(&v["admin"]) && source == "twitch" {
        badges.push("broadcaster".to_string());
    }
    if truthy(&v["moderator"]) {
        badges.push("moderator".to_string());
    }
    if truthy(&v["membership"]) {
        badges.push("subscriber".to_string());
    }
    let id = text_of(&v["id"]);
    let login = text_of(&v["userid"]);
    Some(ChatEvent::Message(ChatMessage {
        id: if id.is_empty() {
            format!("ssn-{}", now_ms())
        } else {
            format!("ssn-{id}")
        },
        platform: Platform::from_type(&source),
        source,
        login: if login.is_empty() {
            author.to_lowercase()
        } else {
            login
        },
        author,
        color: text_of(&v["nameColor"]),
        badges,
        text,
        action: false,
        own: false,
        donation: text_of(&v["hasDonation"]),
        at_ms: now_ms(),
    }))
}

// [LIVE-27]
pub fn send_command(text: &str, target: Option<&str>) -> String {
    let mut cmd = json!({ "action": "sendChat", "value": text });
    if let Some(t) = target.filter(|t| !t.is_empty()) {
        cmd["target"] = Value::String(t.to_string());
    }
    cmd.to_string()
}

enum End {
    Stopped,
    Lost(String),
}

fn would_block(e: &tungstenite::Error) -> bool {
    matches!(e, tungstenite::Error::Io(io) if io.kind() == ErrorKind::WouldBlock)
}

// [LIVE-28]
fn session(
    cfg: &SsnConfig,
    emit: &(dyn Fn(ChatEvent) + Send),
    out: &Receiver<(String, Option<String>)>,
    stop: &AtomicBool,
) -> End {
    let mut ws = match open(cfg) {
        Ok(ws) => ws,
        Err(e) => return End::Lost(format!("connexion impossible : {e}")),
    };
    emit(ChatEvent::Status {
        connected: true,
        writable: true,
        detail: "Social Stream Ninja".into(),
    });
    let mut last_ping = Instant::now();
    loop {
        if stop.load(Ordering::Relaxed) {
            let _ = ws.close(None);
            let _ = ws.flush();
            return End::Stopped;
        }
        loop {
            match ws.read() {
                Ok(Message::Text(t)) => {
                    if let Some(e) = parse_message(t.as_str()) {
                        emit(e);
                    }
                }
                Ok(Message::Close(_)) => return End::Lost("fermée par le serveur".into()),
                Ok(_) => {}
                Err(e) if would_block(&e) => break,
                Err(e) => return End::Lost(format!("connexion perdue : {e}")),
            }
        }
        while let Ok((text, target)) = out.try_recv() {
            match ws.send(Message::text(send_command(&text, target.as_deref()))) {
                Ok(()) => {}
                Err(e) if would_block(&e) => {}
                Err(e) => return End::Lost(format!("envoi impossible : {e}")),
            }
        }
        if last_ping.elapsed() >= PING_EVERY {
            last_ping = Instant::now();
            match ws.send(Message::Ping(Vec::new().into())) {
                Ok(()) => {}
                Err(e) if would_block(&e) => {}
                Err(e) => return End::Lost(format!("connexion perdue : {e}")),
            }
        }
        match ws.flush() {
            Ok(()) => {}
            Err(e) if would_block(&e) => {}
            Err(e) => return End::Lost(format!("connexion perdue : {e}")),
        }
        thread::sleep(IDLE);
    }
}

fn wait(stop: &AtomicBool, secs: u64) {
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until && !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(50));
    }
}

// [LIVE-29]
fn run(
    cfg: SsnConfig,
    emit: Box<dyn Fn(ChatEvent) + Send>,
    out: Receiver<(String, Option<String>)>,
    stop: Arc<AtomicBool>,
) {
    let mut backoff = 1;
    while !stop.load(Ordering::Relaxed) {
        emit(ChatEvent::Status {
            connected: false,
            writable: false,
            detail: "connexion à Social Stream Ninja…".into(),
        });
        let started = Instant::now();
        match session(&cfg, emit.as_ref(), &out, &stop) {
            End::Stopped => break,
            End::Lost(msg) => {
                if started.elapsed() > Duration::from_secs(60) {
                    backoff = 1;
                }
                emit(ChatEvent::Status {
                    connected: false,
                    writable: false,
                    detail: format!("{msg} ; nouvel essai dans {backoff} s"),
                });
                wait(&stop, backoff);
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
    emit(ChatEvent::Status {
        connected: false,
        writable: false,
        detail: "déconnecté".into(),
    });
}

// [LIVE-30]
pub struct SsnClient {
    out: Sender<(String, Option<String>)>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl SsnClient {
    pub fn spawn(config: SsnConfig, emit: impl Fn(ChatEvent) + Send + 'static) -> Self {
        let (out, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = thread::Builder::new()
            .name("blast-ssn".into())
            .spawn(move || run(config, Box::new(emit), rx, flag))
            .ok();
        Self { out, stop, handle }
    }

    pub fn say(&self, text: &str, target: Option<&str>) {
        let _ = self
            .out
            .send((text.to_string(), target.map(str::to_string)));
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for SsnClient {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::Mutex;

    #[test]
    fn session_from_link_or_id() {
        assert_eq!(session_id(" AbC123 "), "AbC123");
        assert_eq!(
            session_id("https://socialstream.ninja/dock.html?session=XyZ789&lang=fr"),
            "XyZ789"
        );
        assert_eq!(
            SsnConfig::new("q1").url(),
            "wss://io.socialstream.ninja:443/join/q1/4/1"
        );
    }

    #[test]
    fn messages_from_many_platforms() {
        let tiktok = parse_message(
            r##"{"chatname":"Lina","chatmessage":"trop bien <img src=\"x.png\" alt=\"🔥\"> &amp; merci","type":"tiktok","id":42,"nameColor":"#FF0050","membership":"Abonné"}"##,
        )
        .unwrap();
        let ChatEvent::Message(m) = tiktok else {
            panic!("message attendu")
        };
        assert_eq!(m.text, "trop bien 🔥 & merci");
        assert_eq!(m.platform, Platform::TikTok);
        assert_eq!(m.source, "tiktok");
        assert_eq!(m.id, "ssn-42");
        assert_eq!(m.login, "lina");
        assert_eq!(m.badges, vec!["subscriber"]);
        let yt = parse_message(r##"{"chatname":"Max","chatmessage":"<b>!np</b>","type":"youtube","textonly":false,"moderator":true,"hasDonation":"5,00 €"}"##).unwrap();
        let ChatEvent::Message(m) = yt else {
            panic!("message attendu")
        };
        assert_eq!(m.text, "!np");
        assert_eq!(m.badges, vec!["moderator"]);
        assert_eq!(m.donation, "5,00 €");
        let plain = parse_message(
            r##"{"chatname":"Bob","chatmessage":"a <b> b","textonly":true,"type":"kick"}"##,
        )
        .unwrap();
        assert!(matches!(plain, ChatEvent::Message(ref m) if m.text == "a <b> b"));
        let follow = parse_message(r##"{"chatname":"Ana","chatmessage":"suit la chaîne","event":"follow","type":"twitch"}"##).unwrap();
        assert_eq!(
            follow,
            ChatEvent::Notice {
                text: "Ana suit la chaîne".into()
            }
        );
        assert!(parse_message(r##"{"action":"something"}"##).is_none());
        assert!(parse_message("pas du json").is_none());
        assert_eq!(
            send_command("salut", Some("tiktok")),
            r##"{"action":"sendChat","target":"tiktok","value":"salut"}"##
        );
    }

    #[test]
    #[allow(clippy::result_large_err)]
    fn receives_and_sends_over_websocket() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (sock, _) = listener.accept().unwrap();
            let mut path = String::new();
            let mut ws = tungstenite::accept_hdr(
                sock,
                |req: &tungstenite::handshake::server::Request, resp| {
                    path = req.uri().path().to_string();
                    Ok(resp)
                },
            )
            .unwrap();
            ws.send(Message::text(
                r##"{"chatname":"Lina","chatmessage":"!request void relay","type":"tiktok"}"##,
            ))
            .unwrap();
            let reply = loop {
                match ws.read().unwrap() {
                    Message::Text(t) => break t.to_string(),
                    _ => continue,
                }
            };
            (path, reply)
        });
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let mut cfg = SsnConfig::new("sess1");
        cfg.host = "127.0.0.1".into();
        cfg.port = port;
        cfg.tls = false;
        let mut client = SsnClient::spawn(cfg, move |e| sink.lock().unwrap().push(e));
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until
            && !events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, ChatEvent::Message(_)))
        {
            thread::sleep(Duration::from_millis(20));
        }
        client.say("@Lina demande reçue", Some("tiktok"));
        let (path, reply) = server.join().unwrap();
        client.stop();
        assert_eq!(path, "/join/sess1/4/1");
        assert_eq!(
            reply,
            r##"{"action":"sendChat","target":"tiktok","value":"@Lina demande reçue"}"##
        );
        let events = events.lock().unwrap();
        assert!(matches!(
            &events[0],
            ChatEvent::Status {
                connected: false,
                ..
            }
        ));
        assert!(matches!(
            &events[1],
            ChatEvent::Status {
                connected: true,
                writable: true,
                ..
            }
        ));
        assert!(
            matches!(&events[2], ChatEvent::Message(m) if m.source == "tiktok" && m.text == "!request void relay")
        );
    }
}
