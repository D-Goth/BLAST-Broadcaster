#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Twitch,
    YouTube,
    Kick,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub platform: Platform,
    pub author: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatCommand {
    Request(String),
    NowPlaying,
    Sfx(String),
}

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
        assert_eq!(parse_command("!request"), None);
        assert_eq!(parse_command("salut"), None);
    }
}
