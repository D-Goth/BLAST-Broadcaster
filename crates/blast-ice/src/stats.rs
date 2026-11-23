use serde::Serialize;
use xmltree::{Element, XMLNode};

use crate::IceError;

// [ICE-21]
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct MountStats {
    pub mount: String,
    pub listeners: u32,
    pub listener_peak: u32,
    pub max_listeners: Option<u32>,
    pub slow_listeners: u32,
    pub bitrate: String,
    pub server_name: String,
    pub server_description: String,
    pub genre: String,
    pub title: String,
    pub server_type: String,
    pub source_ip: String,
    pub user_agent: String,
    pub listenurl: String,
    pub public: bool,
    pub stream_start: Option<i64>,
    pub total_bytes_sent: u64,
    pub total_bytes_read: u64,
    pub connected: bool,
}

// [ICE-22]
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ServerStats {
    pub server_id: String,
    pub host: String,
    pub location: String,
    pub admin: String,
    pub server_start: String,
    pub started_at: Option<i64>,
    pub listeners: u32,
    pub sources: u32,
    pub clients: u32,
    pub client_connections: u64,
    pub listener_connections: u64,
    pub source_client_connections: u64,
    pub file_connections: u64,
    pub mounts: Vec<MountStats>,
}

// [ICE-23]
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Listener {
    pub id: u64,
    pub ip: String,
    pub user_agent: String,
    pub connected_secs: u64,
}

fn text(e: &Element, name: &str) -> String {
    e.get_child(name)
        .and_then(|c| c.get_text())
        .map(|t| t.trim().to_string())
        .unwrap_or_default()
}

fn num<T: std::str::FromStr + Default>(e: &Element, name: &str) -> T {
    text(e, name).parse().unwrap_or_default()
}

fn children<'a>(e: &'a Element, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
    e.children.iter().filter_map(move |n| match n {
        XMLNode::Element(c) if c.name == name => Some(c),
        _ => None,
    })
}

// [ICE-24]
pub fn parse_time(iso: &str, rfc: &str) -> Option<i64> {
    let iso = iso.trim();
    if !iso.is_empty() {
        for f in ["%Y-%m-%dT%H:%M:%S%z", "%Y-%m-%dT%H:%M:%S%:z"] {
            if let Ok(t) = chrono::DateTime::parse_from_str(iso, f) {
                return Some(t.timestamp());
            }
        }
    }
    chrono::DateTime::parse_from_rfc2822(rfc.trim())
        .ok()
        .map(|t| t.timestamp())
}

fn parse_root(xml: &str) -> Result<Element, IceError> {
    let root = Element::parse(xml.as_bytes()).map_err(|e| IceError::Invalid(e.to_string()))?;
    if root.name != "icestats" {
        return Err(IceError::Invalid(format!(
            "balise <{}> au lieu de <icestats>",
            root.name
        )));
    }
    Ok(root)
}

// [ICE-25]
pub fn parse_stats(xml: &str) -> Result<ServerStats, IceError> {
    let r = parse_root(xml)?;
    let mounts = children(&r, "source")
        .map(|s| {
            let title = text(s, "title");
            let artist = text(s, "artist");
            MountStats {
                mount: s.attributes.get("mount").cloned().unwrap_or_default(),
                listeners: num(s, "listeners"),
                listener_peak: num(s, "listener_peak"),
                max_listeners: text(s, "max_listeners").parse().ok(),
                slow_listeners: num(s, "slow_listeners"),
                bitrate: {
                    let b = text(s, "bitrate");
                    if b.is_empty() {
                        text(s, "ice-bitrate")
                    } else {
                        b
                    }
                },
                server_name: text(s, "server_name"),
                server_description: text(s, "server_description"),
                genre: text(s, "genre"),
                title: if artist.is_empty() || title.contains(&artist) {
                    title
                } else {
                    format!("{artist} - {title}")
                },
                server_type: text(s, "server_type"),
                source_ip: text(s, "source_ip"),
                user_agent: text(s, "user_agent"),
                listenurl: text(s, "listenurl"),
                public: text(s, "public") == "1",
                stream_start: parse_time(
                    &text(s, "stream_start_iso8601"),
                    &text(s, "stream_start"),
                ),
                total_bytes_sent: num(s, "total_bytes_sent"),
                total_bytes_read: num(s, "total_bytes_read"),
                connected: !text(s, "source_ip").is_empty() || !text(s, "stream_start").is_empty(),
            }
        })
        .collect();
    Ok(ServerStats {
        server_id: text(&r, "server_id"),
        host: text(&r, "host"),
        location: text(&r, "location"),
        admin: text(&r, "admin"),
        server_start: text(&r, "server_start"),
        started_at: parse_time(&text(&r, "server_start_iso8601"), &text(&r, "server_start")),
        listeners: num(&r, "listeners"),
        sources: num(&r, "sources"),
        clients: num(&r, "clients"),
        client_connections: num(&r, "client_connections"),
        listener_connections: num(&r, "listener_connections"),
        source_client_connections: num(&r, "source_client_connections"),
        file_connections: num(&r, "file_connections"),
        mounts,
    })
}

// [ICE-26]
pub fn parse_listeners(xml: &str) -> Result<Vec<Listener>, IceError> {
    let r = parse_root(xml)?;
    Ok(children(&r, "source")
        .flat_map(|s| children(s, "listener"))
        .map(|l| Listener {
            id: {
                let id: u64 = num(l, "ID");
                if id == 0 {
                    l.attributes
                        .get("id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0)
                } else {
                    id
                }
            },
            ip: text(l, "IP"),
            user_agent: text(l, "UserAgent"),
            connected_secs: num(l, "Connected"),
        })
        .collect())
}

// [ICE-27]
pub fn parse_response(xml: &str) -> Result<String, IceError> {
    let r = Element::parse(xml.as_bytes()).map_err(|e| IceError::Invalid(e.to_string()))?;
    let message = text(&r, "message");
    if text(&r, "return") == "1" {
        Ok(message)
    } else {
        Err(IceError::Refused(if message.is_empty() {
            "sans précision".into()
        } else {
            message
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATS: &str = r#"<?xml version="1.0"?>
<icestats>
  <admin>icemaster@localhost</admin>
  <client_connections>42</client_connections>
  <clients>4</clients>
  <file_connections>3</file_connections>
  <host>localhost</host>
  <listener_connections>12</listener_connections>
  <listeners>2</listeners>
  <location>Earth</location>
  <server_id>Icecast 2.4.4</server_id>
  <server_start>Mon, 23 Nov 2026 08:00:00 +0100</server_start>
  <server_start_iso8601>2026-11-23T08:00:00+0100</server_start_iso8601>
  <source_client_connections>1</source_client_connections>
  <sources>1</sources>
  <source mount="/live">
    <audio_info>channels=2;samplerate=44100;bitrate=128</audio_info>
    <bitrate>128</bitrate>
    <genre>Metal</genre>
    <listener_peak>5</listener_peak>
    <listeners>2</listeners>
    <listenurl>http://localhost:8000/live</listenurl>
    <max_listeners>unlimited</max_listeners>
    <public>0</public>
    <server_name>Black-Lab Radio</server_name>
    <server_type>audio/mpeg</server_type>
    <slow_listeners>0</slow_listeners>
    <source_ip>127.0.0.1</source_ip>
    <stream_start>Mon, 23 Nov 2026 08:05:00 +0100</stream_start>
    <stream_start_iso8601>2026-11-23T08:05:00+0100</stream_start_iso8601>
    <title>D-Goth - Void Angel</title>
    <total_bytes_read>1000</total_bytes_read>
    <total_bytes_sent>2000</total_bytes_sent>
    <user_agent>BLAST-Broadcaster/0.1.0</user_agent>
  </source>
</icestats>"#;

    #[test]
    fn reads_admin_stats() {
        let s = parse_stats(STATS).unwrap();
        assert_eq!(s.server_id, "Icecast 2.4.4");
        assert_eq!(s.listeners, 2);
        assert_eq!(s.started_at, Some(1_795_417_200));
        let m = &s.mounts[0];
        assert_eq!(m.mount, "/live");
        assert_eq!(m.listener_peak, 5);
        assert_eq!(m.max_listeners, None);
        assert_eq!(m.title, "D-Goth - Void Angel");
        assert_eq!(m.stream_start, Some(1_795_417_500));
        assert!(m.connected);
    }

    #[test]
    fn reads_listener_list() {
        let xml = r#"<icestats><source mount="/live"><Listeners>2</Listeners>
            <listener id="7"><IP>192.168.1.20</IP><UserAgent>VLC/3.0.20</UserAgent><Connected>125</Connected><ID>7</ID></listener>
            <listener><IP>10.0.0.3</IP><UserAgent>Mozilla</UserAgent><Connected>4</Connected><ID>9</ID></listener>
            </source></icestats>"#;
        let l = parse_listeners(xml).unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].id, 7);
        assert_eq!(l[0].ip, "192.168.1.20");
        assert_eq!(l[1].connected_secs, 4);
    }

    #[test]
    fn reads_admin_responses() {
        let ok = "<iceresponse><message>Client 7 removed</message><return>1</return></iceresponse>";
        assert_eq!(parse_response(ok).unwrap(), "Client 7 removed");
        let ko = "<iceresponse><message>Client not found</message><return>0</return></iceresponse>";
        assert!(parse_response(ko).is_err());
        assert!(parse_stats("<html/>").is_err());
    }
}
