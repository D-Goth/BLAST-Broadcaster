#[derive(Debug, Clone)]
pub struct ObsConnection {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
}

impl Default for ObsConnection {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 4455,
            password: None,
        }
    }
}

impl ObsConnection {
    pub fn url(&self) -> String {
        format!("ws://{}:{}", self.host, self.port)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url() {
        assert_eq!(ObsConnection::default().url(), "ws://127.0.0.1:4455");
    }
}
