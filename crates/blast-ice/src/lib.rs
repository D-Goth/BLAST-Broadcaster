pub mod client;
pub mod config;
pub mod process;
pub mod stats;

pub use client::{AdminAction, Endpoint};
pub use config::{
    IceConfig, IceDocument, ListenSocket, Logging, Mount, Paths, ServerAuth, ServerLimits,
};
pub use process::{detect, find_in, Install, Process};
pub use stats::{Listener, MountStats, ServerStats};

// [ICE-01]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IceError {
    Unreachable(String),
    Unauthorized,
    Http(u16),
    Invalid(String),
    Refused(String),
    Io(String),
}

impl std::fmt::Display for IceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IceError::Unreachable(e) => write!(f, "serveur injoignable : {e}"),
            IceError::Unauthorized => write!(f, "identifiant ou mot de passe admin refusé"),
            IceError::Http(404) => write!(f, "page introuvable sur le serveur (HTTP 404)"),
            IceError::Http(s) => write!(f, "réponse inattendue du serveur (HTTP {s})"),
            IceError::Invalid(e) => write!(f, "réponse illisible : {e}"),
            IceError::Refused(e) => write!(f, "action refusée par Icecast : {e}"),
            IceError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for IceError {}

impl From<std::io::Error> for IceError {
    fn from(e: std::io::Error) -> Self {
        IceError::Io(e.to_string())
    }
}

// [ICE-02]
pub fn random_password(len: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = vec![0u8; len];
    if getrandom::fill(&mut bytes).is_err() {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (seed >> ((i % 16) * 8)) as u8 ^ (i as u8).wrapping_mul(97);
        }
    }
    bytes
        .iter()
        .map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn passwords_are_random_and_clean() {
        let a = super::random_password(16);
        let b = super::random_password(16);
        assert_eq!(a.len(), 16);
        assert_ne!(a, b);
        assert!(a.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
