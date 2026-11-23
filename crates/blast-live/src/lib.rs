use serde::Serialize;

pub mod irc;
pub mod twitch;

pub use twitch::{TwitchClient, TwitchConfig};

// [LIVE-01]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Twitch,
    YouTube,
    Kick,
}

// [LIVE-02]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatMessage {
    pub id: String,
    pub platform: Platform,
    pub author: String,
    pub login: String,
    pub color: String,
    pub badges: Vec<String>,
    pub text: String,
    pub action: bool,
    pub own: bool,
    pub at_ms: u64,
}

// [LIVE-05]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEvent {
    Status {
        connected: bool,
        writable: bool,
        detail: String,
    },
    Message(ChatMessage),
    Notice {
        text: String,
    },
    Clear {
        login: Option<String>,
    },
    Delete {
        id: String,
    },
}

// [LIVE-03]
#[derive(Debug, Clone, PartialEq)]
pub enum ChatCommand {
    Request(String),
    NowPlaying,
    Sfx(String),
}

// [LIVE-04]
pub fn parse_command(text: &str) -> Option<ChatCommand> {
    let text = text.trim();
    let rest = text.strip_prefix('!')?;
    let (cmd, arg) = match rest.split_once(char::is_whitespace) {
        Some((c, a)) => (c, a.trim()),
        None => (rest, ""),
    };
    match cmd.to_lowercase().as_str() {
        "request" | "sr" if !arg.is_empty() => Some(ChatCommand::Request(arg.to_string())),
        "np" | "song" => Some(ChatCommand::NowPlaying),
        "sfx" if !arg.is_empty() => Some(ChatCommand::Sfx(arg.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands() {
        assert_eq!(
            parse_command("!request Metallica - One"),
            Some(ChatCommand::Request("Metallica - One".into()))
        );
        assert_eq!(parse_command("!NP"), Some(ChatCommand::NowPlaying));
        assert_eq!(
            parse_command("!sr  Nine Inch Nails "),
            Some(ChatCommand::Request("Nine Inch Nails".into()))
        );
        assert_eq!(parse_command("!request"), None);
        assert_eq!(parse_command("salut"), None);
    }
}
