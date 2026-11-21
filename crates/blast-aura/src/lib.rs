pub mod client;

pub use client::{AuraClient, ObsCommand, ObsEvent, ObsInput};

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tungstenite::Message;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuraSettings {
    pub host: String,
    pub port: u16,
    pub auto_connect: bool,
    pub now_playing_source: String,
}

impl Default for AuraSettings {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 4455,
            auto_connect: false,
            now_playing_source: String::new(),
        }
    }
}

impl AuraSettings {
    pub fn url(&self) -> String {
        format!("ws://{}:{}", self.host.trim(), self.port)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AuraAction {
    SetScene(String),
    StartStream,
    StopStream,
    StartRecord,
    StopRecord,
    SetSourceVisible {
        scene: String,
        source: String,
        visible: bool,
    },
    SetText {
        source: String,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ObsInfo {
    pub obs_version: String,
    pub websocket_version: String,
    pub platform: String,
}

#[derive(Debug)]
pub enum AuraError {
    Connect(String),
    Protocol(String),
    PasswordRequired,
    AuthFailed,
}

impl std::fmt::Display for AuraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuraError::Connect(e) => write!(f, "OBS injoignable ({e}). Vérifie qu'OBS est lancé et que le serveur WebSocket est activé (Outils → Paramètres du serveur WebSocket)."),
            AuraError::Protocol(e) => write!(f, "réponse inattendue d'OBS : {e}"),
            AuraError::PasswordRequired => write!(f, "OBS demande un mot de passe."),
            AuraError::AuthFailed => write!(f, "mot de passe OBS refusé."),
        }
    }
}

impl std::error::Error for AuraError {}

pub fn auth_response(password: &str, salt: &str, challenge: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = b64.encode(Sha256::digest(format!("{password}{salt}").as_bytes()));
    b64.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}

pub(crate) fn read_json(ws: &mut tungstenite::WebSocket<TcpStream>) -> Result<Value, AuraError> {
    loop {
        match ws.read() {
            Ok(Message::Text(t)) => {
                return serde_json::from_str(t.as_str())
                    .map_err(|e| AuraError::Protocol(e.to_string()))
            }
            Ok(Message::Close(frame)) => {
                let code: u16 = frame.as_ref().map(|f| f.code.into()).unwrap_or(0);
                return Err(if code == 4009 {
                    AuraError::AuthFailed
                } else {
                    AuraError::Protocol(format!(
                        "connexion fermée ({code} {})",
                        frame.map(|f| f.reason.to_string()).unwrap_or_default()
                    ))
                });
            }
            Ok(_) => continue,
            Err(e) => return Err(AuraError::Protocol(e.to_string())),
        }
    }
}

pub fn test_connection(
    settings: &AuraSettings,
    password: Option<&str>,
) -> Result<ObsInfo, AuraError> {
    let addr = (settings.host.trim(), settings.port)
        .to_socket_addrs()
        .map_err(|e| AuraError::Connect(e.to_string()))?
        .next()
        .ok_or_else(|| AuraError::Connect("adresse inconnue".into()))?;
    let stream = TcpStream::connect_timeout(&addr, Duration::from_secs(4))
        .map_err(|e| AuraError::Connect(e.to_string()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| AuraError::Connect(e.to_string()))?;
    let (mut ws, _) = tungstenite::client(settings.url(), stream)
        .map_err(|e| AuraError::Connect(e.to_string()))?;

    let hello = read_json(&mut ws)?;
    if hello["op"] != 0 {
        return Err(AuraError::Protocol("message Hello absent".into()));
    }
    let websocket_version = hello["d"]["obsWebSocketVersion"]
        .as_str()
        .unwrap_or("?")
        .to_string();
    let mut identify = json!({"op": 1, "d": {"rpcVersion": 1, "eventSubscriptions": 0}});
    if let Some(auth) = hello["d"].get("authentication") {
        let pw = password
            .filter(|p| !p.is_empty())
            .ok_or(AuraError::PasswordRequired)?;
        let salt = auth["salt"].as_str().unwrap_or_default();
        let challenge = auth["challenge"].as_str().unwrap_or_default();
        identify["d"]["authentication"] = Value::String(auth_response(pw, salt, challenge));
    }
    ws.send(Message::text(identify.to_string()))
        .map_err(|e| AuraError::Protocol(e.to_string()))?;
    let identified = read_json(&mut ws)?;
    if identified["op"] != 2 {
        return Err(AuraError::Protocol(format!(
            "op {} au lieu de Identified",
            identified["op"]
        )));
    }
    ws.send(Message::text(
        json!({"op": 6, "d": {"requestType": "GetVersion", "requestId": "blast-test"}}).to_string(),
    ))
    .map_err(|e| AuraError::Protocol(e.to_string()))?;
    let mut info = ObsInfo {
        obs_version: "?".into(),
        websocket_version,
        platform: String::new(),
    };
    for _ in 0..8 {
        let msg = read_json(&mut ws)?;
        if msg["op"] == 7 && msg["d"]["requestId"] == "blast-test" {
            let data = &msg["d"]["responseData"];
            info.obs_version = data["obsVersion"].as_str().unwrap_or("?").to_string();
            info.platform = data["platformDescription"]
                .as_str()
                .unwrap_or("")
                .to_string();
            break;
        }
    }
    let _ = ws.close(None);
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn default_url() {
        assert_eq!(AuraSettings::default().url(), "ws://127.0.0.1:4455");
    }

    #[test]
    fn auth_matches_protocol_example() {
        assert_eq!(
            auth_response(
                "supersecretpassword",
                "lM1GncleQOaCu9lT1yeUZhFYnqhsLLP1G5lAGo3ixaI=",
                "+IxH4CnCiqpX1rM9scsNynZzbOe4KhDeYcTNS3PDaeY="
            ),
            "1Ct943GAT+6YQUUX47Ia/ncufilbe6+oD6lY+5kaCu4="
        );
    }

    fn fake_obs(password: &'static str) -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut ws = tungstenite::accept(s).unwrap();
            let salt = "c2FsdA==";
            let challenge = "Y2hhbGxlbmdl";
            ws.send(Message::text(
                json!({"op":0,"d":{"obsWebSocketVersion":"5.5.0","rpcVersion":1,
                    "authentication":{"salt":salt,"challenge":challenge}}})
                .to_string(),
            ))
            .unwrap();
            let id: Value = match ws.read().unwrap() {
                Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(),
                _ => panic!(),
            };
            if id["d"]["authentication"] != auth_response(password, salt, challenge) {
                let _ = ws.close(Some(tungstenite::protocol::CloseFrame {
                    code: 4009u16.into(),
                    reason: "Authentication failed.".into(),
                }));
                let _ = ws.flush();
                let _ = ws.read();
                return;
            }
            ws.send(Message::text(
                json!({"op":2,"d":{"negotiatedRpcVersion":1}}).to_string(),
            ))
            .unwrap();
            let _ = ws.read().unwrap();
            ws.send(Message::text(
                json!({"op":7,"d":{"requestType":"GetVersion","requestId":"blast-test",
                    "requestStatus":{"result":true,"code":100},
                    "responseData":{"obsVersion":"31.0.0","platformDescription":"Kubuntu"}}})
                .to_string(),
            ))
            .unwrap();
            let _ = ws.read();
        });
        port
    }

    #[test]
    fn connects_with_password() {
        let port = fake_obs("hunter2");
        let s = AuraSettings {
            port,
            ..Default::default()
        };
        let info = test_connection(&s, Some("hunter2")).unwrap();
        assert_eq!(info.obs_version, "31.0.0");
        assert_eq!(info.websocket_version, "5.5.0");
    }

    #[test]
    fn wrong_password_is_reported() {
        let port = fake_obs("hunter2");
        let s = AuraSettings {
            port,
            ..Default::default()
        };
        assert!(matches!(
            test_connection(&s, Some("nope")),
            Err(AuraError::AuthFailed)
        ));
    }

    #[test]
    fn missing_password_is_reported() {
        let port = fake_obs("hunter2");
        let s = AuraSettings {
            port,
            ..Default::default()
        };
        assert!(matches!(
            test_connection(&s, None),
            Err(AuraError::PasswordRequired)
        ));
    }
}
