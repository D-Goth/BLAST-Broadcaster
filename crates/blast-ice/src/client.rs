use std::time::Duration;

use base64::Engine;
use serde::Deserialize;

use crate::stats::{parse_listeners, parse_response, parse_stats, Listener, ServerStats};
use crate::IceError;

// [ICE-28]
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum AdminAction {
    KillClient { mount: String, id: u64 },
    KillSource { mount: String },
    MoveClients { mount: String, destination: String },
    Metadata { mount: String, song: String },
}

// [ICE-29]
#[derive(Debug, Clone, PartialEq)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
    pub https: bool,
    pub user: String,
    pub password: String,
    pub user_agent: String,
}

// [ICE-30]
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

// [ICE-43]
pub fn error_message(body: &str) -> Option<String> {
    let inner = body.split_once("<body>").map(|(_, b)| b).unwrap_or(body);
    let mut text = String::new();
    let mut in_tag = false;
    for c in inner.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text.trim();
    let text = text
        .split_once(" - ")
        .filter(|(code, _)| code.chars().all(|c| c.is_ascii_digit()))
        .map(|(_, m)| m)
        .unwrap_or(text)
        .trim();
    (!text.is_empty() && text.len() < 300).then(|| text.to_string())
}

impl Endpoint {
    pub fn base(&self) -> String {
        let scheme = if self.https { "https" } else { "http" };
        let host = self.host.trim();
        let host = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        format!("{scheme}://{host}:{}", self.port)
    }

    fn agent(timeout: Duration) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .into()
    }

    // [ICE-31]
    fn get(&self, path: &str, admin: bool) -> Result<String, IceError> {
        let url = format!("{}{path}", self.base());
        let mut req = Self::agent(Duration::from_secs(5))
            .get(&url)
            .header("User-Agent", &self.user_agent);
        if admin {
            let auth = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", self.user, self.password));
            req = req.header("Authorization", &format!("Basic {auth}"));
        }
        let mut resp = req
            .call()
            .map_err(|e| IceError::Unreachable(e.to_string()))?;
        match resp.status().as_u16() {
            200 => resp
                .body_mut()
                .with_config()
                .limit(8 * 1024 * 1024)
                .read_to_string()
                .map_err(|e| IceError::Invalid(e.to_string())),
            401 | 403 => Err(IceError::Unauthorized),
            400 => {
                let body = resp.body_mut().read_to_string().unwrap_or_default();
                Err(error_message(&body)
                    .map(IceError::Refused)
                    .unwrap_or(IceError::Http(400)))
            }
            s => Err(IceError::Http(s)),
        }
    }

    // [ICE-32]
    pub fn probe(&self) -> Result<String, IceError> {
        let body = self.get("/status-json.xsl", false)?;
        let id = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v["icestats"]["server_id"].as_str().map(String::from));
        Ok(id.unwrap_or_else(|| "Icecast".into()))
    }

    // [ICE-33]
    pub fn stats(&self) -> Result<ServerStats, IceError> {
        parse_stats(&self.get("/admin/stats", true)?)
    }

    // [ICE-34]
    pub fn listeners(&self, mount: &str) -> Result<Vec<Listener>, IceError> {
        parse_listeners(&self.get(&format!("/admin/listclients?mount={}", encode(mount)), true)?)
    }

    // [ICE-35]
    pub fn run(&self, action: &AdminAction) -> Result<String, IceError> {
        let path = match action {
            AdminAction::KillClient { mount, id } => {
                format!("/admin/killclient?mount={}&id={id}", encode(mount))
            }
            AdminAction::KillSource { mount } => {
                format!("/admin/killsource?mount={}", encode(mount))
            }
            AdminAction::MoveClients { mount, destination } => format!(
                "/admin/moveclients?mount={}&destination={}",
                encode(mount),
                encode(destination)
            ),
            AdminAction::Metadata { mount, song } => format!(
                "/admin/metadata?mount={}&mode=updinfo&song={}",
                encode(mount),
                encode(song)
            ),
        };
        let body = self.get(&path, true)?;
        if body.trim_start().starts_with('<') {
            parse_response(&body)
        } else {
            Ok(body.trim().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    fn endpoint(port: u16) -> Endpoint {
        Endpoint {
            host: "127.0.0.1".into(),
            port,
            https: false,
            user: "admin".into(),
            password: "secret".into(),
            user_agent: "test".into(),
        }
    }

    fn serve(status: &'static str, body: &'static str) -> (u16, std::thread::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let h = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut head = String::new();
            let mut r = BufReader::new(s.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if r.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let _ = write!(
                s,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            head
        });
        (port, h)
    }

    #[test]
    fn reads_html_error_pages() {
        let page = "<html><head><title>Error 400</title></head><body><b>400 - Source does not exist</b></body></html>";
        assert_eq!(
            error_message(page).as_deref(),
            Some("Source does not exist")
        );
        assert_eq!(error_message(""), None);
    }

    #[test]
    fn encodes_query_values() {
        assert_eq!(encode("/live"), "/live");
        assert_eq!(encode("AC/DC & Co"), "AC/DC%20%26%20Co");
        assert_eq!(encode("é"), "%C3%A9");
    }

    #[test]
    fn sends_basic_auth_and_reads_actions() {
        let (port, h) = serve(
            "200 OK",
            "<iceresponse><message>Metadata update successful</message><return>1</return></iceresponse>",
        );
        let msg = endpoint(port)
            .run(&AdminAction::Metadata {
                mount: "/live".into(),
                song: "A & B".into(),
            })
            .unwrap();
        assert_eq!(msg, "Metadata update successful");
        let head = h.join().unwrap();
        assert!(head.starts_with("GET /admin/metadata?mount=/live&mode=updinfo&song=A%20%26%20B "));
        assert!(head.contains("Basic YWRtaW46c2VjcmV0"));
    }

    #[test]
    fn maps_refused_credentials() {
        let (port, h) = serve("401 Unauthorized", "");
        assert_eq!(endpoint(port).stats(), Err(IceError::Unauthorized));
        h.join().unwrap();
    }

    #[test]
    fn unreachable_server() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        drop(l);
        assert!(matches!(
            endpoint(port).probe(),
            Err(IceError::Unreachable(_))
        ));
    }

    #[test]
    fn ipv6_hosts_are_bracketed() {
        let mut e = endpoint(8000);
        e.host = "::1".into();
        e.https = true;
        assert_eq!(e.base(), "https://[::1]:8000");
    }
}
