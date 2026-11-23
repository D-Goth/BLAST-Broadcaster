use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::irc::{parse_line, IrcMessage};
use crate::{ChatEvent, ChatMessage, Platform};

// [LIVE-09]
const IDLE: Duration = Duration::from_millis(15);
const SEND_GAP: Duration = Duration::from_millis(1600);
const SILENCE: Duration = Duration::from_secs(360);
const MAX_BACKOFF: u64 = 30;

// [LIVE-10]
#[derive(Debug, Clone, PartialEq)]
pub struct TwitchConfig {
    pub channel: String,
    pub login: String,
    pub token: String,
    pub host: String,
    pub port: u16,
    pub tls: bool,
}

impl TwitchConfig {
    pub fn new(channel: &str, login: &str, token: &str) -> Self {
        Self {
            channel: normalize_channel(channel),
            login: login.trim().to_lowercase(),
            token: token.trim().trim_start_matches("oauth:").to_string(),
            host: "irc.chat.twitch.tv".into(),
            port: 6697,
            tls: true,
        }
    }

    pub fn writable(&self) -> bool {
        !self.token.is_empty() && !self.login.is_empty()
    }
}

// [LIVE-11]
pub fn normalize_channel(input: &str) -> String {
    let s = input.trim();
    let s = s
        .rsplit_once("twitch.tv/")
        .map(|(_, rest)| rest)
        .unwrap_or(s);
    s.trim_start_matches('#')
        .split(['/', '?', ' '])
        .next()
        .unwrap_or("")
        .to_lowercase()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// [LIVE-12]
enum Transport {
    Plain(TcpStream),
    Tls(Box<rustls::ClientConnection>, TcpStream),
}

impl Transport {
    fn open(cfg: &TwitchConfig) -> io::Result<Self> {
        let addr = (cfg.host.as_str(), cfg.port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "adresse introuvable"))?;
        let sock = TcpStream::connect_timeout(&addr, Duration::from_secs(6))?;
        sock.set_nodelay(true)?;
        sock.set_nonblocking(true)?;
        if !cfg.tls {
            return Ok(Transport::Plain(sock));
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier =
            rustls_platform_verifier::Verifier::new(provider.clone()).map_err(io::Error::other)?;
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(io::Error::other)?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(verifier))
            .with_no_client_auth();
        let name =
            rustls::pki_types::ServerName::try_from(cfg.host.clone()).map_err(io::Error::other)?;
        let conn =
            rustls::ClientConnection::new(Arc::new(config), name).map_err(io::Error::other)?;
        Ok(Transport::Tls(Box::new(conn), sock))
    }

    fn read_into(&mut self, buf: &mut Vec<u8>) -> io::Result<bool> {
        let mut tmp = [0u8; 8192];
        let mut got = false;
        match self {
            Transport::Plain(sock) => loop {
                match sock.read(&mut tmp) {
                    Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        got = true;
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(got),
                    Err(e) if e.kind() == ErrorKind::Interrupted => {}
                    Err(e) => return Err(e),
                }
            },
            Transport::Tls(conn, sock) => {
                loop {
                    match conn.read_tls(sock) {
                        Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
                        Ok(_) => {
                            conn.process_new_packets().map_err(io::Error::other)?;
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == ErrorKind::Interrupted => {}
                        Err(e) => return Err(e),
                    }
                }
                flush_tls(conn, sock)?;
                loop {
                    match conn.reader().read(&mut tmp) {
                        Ok(0) => return Err(ErrorKind::UnexpectedEof.into()),
                        Ok(n) => {
                            buf.extend_from_slice(&tmp[..n]);
                            got = true;
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(got),
                        Err(e) => return Err(e),
                    }
                }
            }
        }
    }

    fn send(&mut self, line: &str) -> io::Result<()> {
        let data = format!("{line}\r\n");
        match self {
            Transport::Plain(sock) => {
                let mut rest = data.as_bytes();
                let deadline = Instant::now() + Duration::from_secs(5);
                while !rest.is_empty() {
                    match sock.write(rest) {
                        Ok(n) => rest = &rest[n..],
                        Err(e) if e.kind() == ErrorKind::WouldBlock => {
                            if Instant::now() > deadline {
                                return Err(ErrorKind::TimedOut.into());
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(e) if e.kind() == ErrorKind::Interrupted => {}
                        Err(e) => return Err(e),
                    }
                }
                Ok(())
            }
            Transport::Tls(conn, sock) => {
                conn.writer().write_all(data.as_bytes())?;
                flush_tls(conn, sock)
            }
        }
    }
}

fn flush_tls(conn: &mut rustls::ClientConnection, sock: &mut TcpStream) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while conn.wants_write() {
        match conn.write_tls(sock) {
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    return Err(ErrorKind::TimedOut.into());
                }
                thread::sleep(Duration::from_millis(2));
            }
            Err(e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

// [LIVE-13]
enum End {
    Stopped,
    Reconnect,
    Lost(String),
    Fatal(String),
}

#[derive(Default)]
struct Me {
    name: String,
    color: String,
    badges: Vec<String>,
}

// [LIVE-14]
fn badges(tag: &str) -> Vec<String> {
    tag.split(',')
        .filter_map(|b| b.split('/').next())
        .filter(|b| !b.is_empty())
        .map(str::to_string)
        .collect()
}

// [LIVE-15]
fn to_message(m: &IrcMessage) -> ChatMessage {
    let raw = m.trailing();
    let (text, action) = match raw
        .strip_prefix("\u{1}ACTION ")
        .and_then(|t| t.strip_suffix('\u{1}'))
    {
        Some(t) => (t.to_string(), true),
        None => (raw.to_string(), false),
    };
    let login = m.nick().to_string();
    let display = m.tag("display-name");
    ChatMessage {
        id: m.tag("id").to_string(),
        platform: Platform::Twitch,
        author: if display.is_empty() {
            login.clone()
        } else {
            display.to_string()
        },
        login,
        color: m.tag("color").to_string(),
        badges: badges(m.tag("badges")),
        text,
        action,
        own: false,
        at_ms: m.tag("tmi-sent-ts").parse().unwrap_or_else(|_| now_ms()),
    }
}

// [LIVE-16]
fn session(
    cfg: &TwitchConfig,
    emit: &(dyn Fn(ChatEvent) + Send),
    out: &Receiver<String>,
    stop: &AtomicBool,
) -> End {
    let mut t = match Transport::open(cfg) {
        Ok(t) => t,
        Err(e) => return End::Lost(format!("connexion impossible : {e}")),
    };
    let writable = cfg.writable();
    let nick = if writable {
        cfg.login.clone()
    } else {
        format!("justinfan{}", 10_000 + now_ms() % 89_999)
    };
    let mut hello = vec!["CAP REQ :twitch.tv/tags twitch.tv/commands".to_string()];
    if writable {
        hello.push(format!("PASS oauth:{}", cfg.token));
    }
    hello.push(format!("NICK {nick}"));
    hello.push(format!("JOIN #{}", cfg.channel));
    for line in &hello {
        if let Err(e) = t.send(line) {
            return End::Lost(format!("envoi impossible : {e}"));
        }
    }
    let mut buf = Vec::new();
    let mut last_rx = Instant::now();
    let mut last_tx = Instant::now() - SEND_GAP;
    let mut joined = false;
    let mut queue: Vec<String> = Vec::new();
    let mut me = Me {
        name: cfg.login.clone(),
        ..Default::default()
    };
    loop {
        if stop.load(Ordering::Relaxed) {
            return End::Stopped;
        }
        match t.read_into(&mut buf) {
            Ok(true) => last_rx = Instant::now(),
            Ok(false) => {}
            Err(e) => return End::Lost(format!("connexion perdue : {e}")),
        }
        while let Some(pos) = buf.windows(2).position(|w| w == b"\r\n") {
            let line: Vec<u8> = buf.drain(..pos + 2).collect();
            let line = String::from_utf8_lossy(&line);
            let Some(m) = parse_line(&line) else {
                continue;
            };
            match m.command.as_str() {
                "PING" => {
                    if let Err(e) = t.send(&format!("PONG :{}", m.trailing())) {
                        return End::Lost(format!("connexion perdue : {e}"));
                    }
                }
                "RECONNECT" => return End::Reconnect,
                "ROOMSTATE" | "366" if !joined => {
                    joined = true;
                    emit(ChatEvent::Status {
                        connected: true,
                        writable,
                        detail: format!("#{}", cfg.channel),
                    });
                }
                "PRIVMSG" => emit(ChatEvent::Message(to_message(&m))),
                "USERNOTICE" => {
                    let system = m.tag("system-msg");
                    let said = if m.params.len() > 1 { m.trailing() } else { "" };
                    let text = match (system.is_empty(), said.is_empty()) {
                        (false, false) => format!("{system} — {said}"),
                        (false, true) => system.to_string(),
                        _ => said.to_string(),
                    };
                    if !text.is_empty() {
                        emit(ChatEvent::Notice { text });
                    }
                }
                "USERSTATE" | "GLOBALUSERSTATE" => {
                    let display = m.tag("display-name");
                    if !display.is_empty() {
                        me.name = display.to_string();
                    }
                    me.color = m.tag("color").to_string();
                    me.badges = badges(m.tag("badges"));
                }
                "CLEARCHAT" => emit(ChatEvent::Clear {
                    login: m.params.get(1).cloned(),
                }),
                "CLEARMSG" => emit(ChatEvent::Delete {
                    id: m.tag("target-msg-id").to_string(),
                }),
                "NOTICE" => {
                    let text = m.trailing().to_string();
                    let low = text.to_lowercase();
                    if low.contains("authentication failed")
                        || low.contains("improperly formatted auth")
                    {
                        return End::Fatal(
                            "Jeton refusé par Twitch : vérifie le compte et le jeton dans Réglages → Chat."
                                .into(),
                        );
                    }
                    emit(ChatEvent::Notice { text });
                }
                _ => {}
            }
        }
        while let Ok(text) = out.try_recv() {
            if writable {
                queue.push(text);
            } else {
                emit(ChatEvent::Notice {
                    text: "Chat en lecture seule : ajoute un compte et un jeton dans Réglages → Chat pour écrire.".into(),
                });
            }
        }
        if joined && !queue.is_empty() && last_tx.elapsed() >= SEND_GAP {
            let text = queue.remove(0);
            let text: String = text.replace(['\r', '\n'], " ").chars().take(480).collect();
            if let Err(e) = t.send(&format!("PRIVMSG #{} :{}", cfg.channel, text)) {
                return End::Lost(format!("connexion perdue : {e}"));
            }
            last_tx = Instant::now();
            emit(ChatEvent::Message(ChatMessage {
                id: format!("own-{}", now_ms()),
                platform: Platform::Twitch,
                author: me.name.clone(),
                login: cfg.login.clone(),
                color: me.color.clone(),
                badges: me.badges.clone(),
                text,
                action: false,
                own: true,
                at_ms: now_ms(),
            }));
        }
        if last_rx.elapsed() > SILENCE {
            return End::Lost("plus de nouvelles de Twitch depuis 6 min".into());
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

// [LIVE-17]
fn run(
    cfg: TwitchConfig,
    emit: Box<dyn Fn(ChatEvent) + Send>,
    out: Receiver<String>,
    stop: Arc<AtomicBool>,
) {
    let writable = cfg.writable();
    let mut backoff = 1;
    while !stop.load(Ordering::Relaxed) {
        emit(ChatEvent::Status {
            connected: false,
            writable,
            detail: format!("connexion à #{}…", cfg.channel),
        });
        let started = Instant::now();
        match session(&cfg, emit.as_ref(), &out, &stop) {
            End::Stopped => break,
            End::Reconnect => backoff = 1,
            End::Fatal(msg) => {
                emit(ChatEvent::Status {
                    connected: false,
                    writable,
                    detail: msg,
                });
                while !stop.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(100));
                }
                return;
            }
            End::Lost(msg) => {
                if started.elapsed() > Duration::from_secs(60) {
                    backoff = 1;
                }
                emit(ChatEvent::Status {
                    connected: false,
                    writable,
                    detail: format!("{msg} ; nouvel essai dans {backoff} s"),
                });
                wait(&stop, backoff);
                backoff = (backoff * 2).min(MAX_BACKOFF);
            }
        }
    }
    emit(ChatEvent::Status {
        connected: false,
        writable,
        detail: "déconnecté".into(),
    });
}

// [LIVE-18]
pub struct TwitchClient {
    out: Sender<String>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    pub config: TwitchConfig,
}

impl TwitchClient {
    pub fn spawn(config: TwitchConfig, emit: impl Fn(ChatEvent) + Send + 'static) -> Self {
        let (out, rx) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let cfg = config.clone();
        let handle = thread::Builder::new()
            .name("blast-twitch".into())
            .spawn(move || run(cfg, Box::new(emit), rx, flag))
            .ok();
        Self {
            out,
            stop,
            handle,
            config,
        }
    }

    pub fn say(&self, text: &str) {
        let _ = self.out.send(text.to_string());
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for TwitchClient {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::sync::Mutex;

    fn read_until(r: &mut BufReader<TcpStream>, prefix: &str, seen: &mut Vec<String>) {
        loop {
            let mut line = String::new();
            assert!(
                r.read_line(&mut line).unwrap() > 0,
                "fermé avant « {prefix} »"
            );
            let line = line.trim_end().to_string();
            seen.push(line.clone());
            if line.starts_with(prefix) {
                return;
            }
        }
    }

    fn wait_for(events: &Mutex<Vec<ChatEvent>>, pred: impl Fn(&ChatEvent) -> bool) {
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            if events.lock().unwrap().iter().any(&pred) {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("événement attendu absent : {:?}", events.lock().unwrap());
    }

    #[test]
    fn channel_names() {
        assert_eq!(normalize_channel(" #Black_Lab "), "black_lab");
        assert_eq!(
            normalize_channel("https://www.twitch.tv/D_Goth?x=1"),
            "d_goth"
        );
        assert_eq!(normalize_channel("twitch.tv/dgoth/videos"), "dgoth");
    }

    #[test]
    fn reads_writes_and_reconnects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let server = thread::spawn(move || {
            let (sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut w = sock.try_clone().unwrap();
            let mut r = BufReader::new(sock);
            let mut lines = Vec::new();
            read_until(&mut r, "JOIN", &mut lines);
            w.write_all(
                b"@badges=;color=#00FF00;display-name=BlastBot :tmi.twitch.tv GLOBALUSERSTATE\r\n",
            )
            .unwrap();
            w.write_all(b"@emote-only=0;room-id=1 :tmi.twitch.tv ROOMSTATE #blacklab\r\n")
                .unwrap();
            w.write_all(b"@badges=moderator/1,subscriber/6;color=#FF0000;display-name=Viewer;id=m1;tmi-sent-ts=1700000000000 :viewer!viewer@viewer.tmi.twitch.tv PRIVMSG #blacklab :!np\r\n").unwrap();
            w.write_all(
                b"@id=m2 :fan!fan@fan.tmi.twitch.tv PRIVMSG #blacklab :\x01ACTION danse\x01\r\n",
            )
            .unwrap();
            w.write_all(b"PING :tmi.twitch.tv\r\n").unwrap();
            read_until(&mut r, "PONG", &mut lines);
            read_until(&mut r, "PRIVMSG", &mut lines);
            w.write_all(b":tmi.twitch.tv RECONNECT\r\n").unwrap();
            let (sock2, _) = listener.accept().unwrap();
            sock2
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut w2 = sock2.try_clone().unwrap();
            let mut r2 = BufReader::new(sock2);
            read_until(&mut r2, "JOIN", &mut lines);
            w2.write_all(b":tmi.twitch.tv ROOMSTATE #blacklab\r\n")
                .unwrap();
            log.lock().unwrap().extend(lines);
            thread::sleep(Duration::from_millis(500));
        });

        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let mut cfg = TwitchConfig::new("#BlackLab", "BlastBot", "oauth:secret");
        cfg.host = "127.0.0.1".into();
        cfg.port = port;
        cfg.tls = false;
        let mut client = TwitchClient::spawn(cfg, move |e| sink.lock().unwrap().push(e));
        wait_for(&events, |e| {
            matches!(
                e,
                ChatEvent::Status {
                    connected: true,
                    writable: true,
                    ..
                }
            )
        });
        client.say("Titre en cours : D-Goth - Void Relay");
        server.join().unwrap();
        wait_for(&events, |e| {
            matches!(
                e,
                ChatEvent::Status {
                    connected: true,
                    ..
                }
            )
        });
        client.stop();

        let lines = seen.lock().unwrap().clone();
        assert!(lines.contains(&"PASS oauth:secret".to_string()));
        assert!(lines.contains(&"NICK blastbot".to_string()));
        assert!(lines.contains(&"JOIN #blacklab".to_string()));
        assert!(lines.contains(&"PONG :tmi.twitch.tv".to_string()));
        assert!(
            lines.contains(&"PRIVMSG #blacklab :Titre en cours : D-Goth - Void Relay".to_string())
        );
        assert_eq!(lines.iter().filter(|l| l.starts_with("JOIN")).count(), 2);

        let events = events.lock().unwrap();
        let msgs: Vec<&ChatMessage> = events
            .iter()
            .filter_map(|e| match e {
                ChatEvent::Message(m) => Some(m),
                _ => None,
            })
            .collect();
        assert_eq!(msgs[0].author, "Viewer");
        assert_eq!(msgs[0].badges, vec!["moderator", "subscriber"]);
        assert_eq!(msgs[0].at_ms, 1_700_000_000_000);
        assert_eq!(msgs[1].text, "danse");
        assert!(msgs[1].action);
        assert_eq!(msgs[1].author, "fan");
        assert!(msgs[2].own);
        assert_eq!(msgs[2].author, "BlastBot");
        assert_eq!(msgs[2].color, "#00FF00");
    }

    #[test]
    fn anonymous_reading_uses_justinfan() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut w = sock.try_clone().unwrap();
            let mut r = BufReader::new(sock);
            let mut lines = Vec::new();
            read_until(&mut r, "JOIN", &mut lines);
            w.write_all(b":tmi.twitch.tv ROOMSTATE #blacklab\r\n")
                .unwrap();
            thread::sleep(Duration::from_millis(300));
            lines
        });
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        let mut cfg = TwitchConfig::new("blacklab", "", "");
        cfg.host = "127.0.0.1".into();
        cfg.port = port;
        cfg.tls = false;
        let mut client = TwitchClient::spawn(cfg, move |e| sink.lock().unwrap().push(e));
        wait_for(&events, |e| {
            matches!(
                e,
                ChatEvent::Status {
                    connected: true,
                    writable: false,
                    ..
                }
            )
        });
        client.say("bonjour");
        let lines = server.join().unwrap();
        client.stop();
        assert!(lines.iter().any(|l| l.starts_with("NICK justinfan")));
        assert!(!lines.iter().any(|l| l.starts_with("PASS")));
        let events = events.lock().unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, ChatEvent::Notice { text } if text.contains("lecture seule"))));
    }
}
