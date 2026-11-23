use std::collections::HashMap;

// [LIVE-06]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IrcMessage {
    pub tags: HashMap<String, String>,
    pub prefix: String,
    pub command: String,
    pub params: Vec<String>,
}

impl IrcMessage {
    pub fn tag(&self, name: &str) -> &str {
        self.tags.get(name).map(String::as_str).unwrap_or("")
    }

    pub fn nick(&self) -> &str {
        self.prefix.split('!').next().unwrap_or("")
    }

    pub fn trailing(&self) -> &str {
        self.params.last().map(String::as_str).unwrap_or("")
    }
}

// [LIVE-07]
fn unescape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(':') => out.push(';'),
            Some('s') => out.push(' '),
            Some('\\') => out.push('\\'),
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

// [LIVE-08]
pub fn parse_line(line: &str) -> Option<IrcMessage> {
    let mut rest = line.trim_end_matches(['\r', '\n']);
    if rest.is_empty() {
        return None;
    }
    let mut msg = IrcMessage::default();
    if let Some(r) = rest.strip_prefix('@') {
        let (tags, after) = r.split_once(' ')?;
        for pair in tags.split(';') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            msg.tags.insert(k.to_string(), unescape(v));
        }
        rest = after.trim_start();
    }
    if let Some(r) = rest.strip_prefix(':') {
        let (prefix, after) = r.split_once(' ').unwrap_or((r, ""));
        msg.prefix = prefix.to_string();
        rest = after.trim_start();
    }
    let (head, trailing) = match rest.split_once(" :") {
        Some((h, t)) => (h, Some(t)),
        None => (rest, None),
    };
    let mut words = head.split_whitespace();
    msg.command = words.next()?.to_ascii_uppercase();
    msg.params = words.map(str::to_string).collect();
    if let Some(t) = trailing {
        msg.params.push(t.to_string());
    }
    Some(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privmsg_with_tags() {
        let m = parse_line(
            "@badges=moderator/1,subscriber/12;color=#1E90FF;display-name=Loïc\\sD;id=b34;tmi-sent-ts=1700000000000 :dgoth!dgoth@dgoth.tmi.twitch.tv PRIVMSG #blacklab :salut : tout le monde\r\n",
        )
        .unwrap();
        assert_eq!(m.command, "PRIVMSG");
        assert_eq!(m.nick(), "dgoth");
        assert_eq!(m.params, vec!["#blacklab", "salut : tout le monde"]);
        assert_eq!(m.tag("display-name"), "Loïc D");
        assert_eq!(m.tag("color"), "#1E90FF");
        assert_eq!(m.tag("absent"), "");
    }

    #[test]
    fn simple_lines() {
        let ping = parse_line("PING :tmi.twitch.tv").unwrap();
        assert_eq!(ping.command, "PING");
        assert_eq!(ping.trailing(), "tmi.twitch.tv");
        let clear = parse_line("@room-id=1 :tmi.twitch.tv CLEARCHAT #chan :troll").unwrap();
        assert_eq!(clear.params, vec!["#chan", "troll"]);
        let all = parse_line(":tmi.twitch.tv CLEARCHAT #chan").unwrap();
        assert_eq!(all.params, vec!["#chan"]);
        assert!(parse_line("\r\n").is_none());
    }
}
