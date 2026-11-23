use std::collections::HashMap;
use std::sync::Mutex;

// [SEC-01]
const SERVICE: &str = "fr.black-lab.blast";

// [SEC-02]
pub enum Stored {
    Keyring,
    Memory(String),
}

// [SEC-03]
#[derive(Default)]
pub struct Secrets {
    memory: Mutex<HashMap<String, String>>,
    last_error: Mutex<Option<String>>,
}

// [SEC-04]
pub fn output_key(id: &str) -> String {
    format!("output:{id}")
}

// [SEC-05]
pub const AURA_KEY: &str = "aura";

// [SEC-08]
pub const CHAT_KEY: &str = "twitch";

// [SEC-07]
pub fn ice_key(id: &str) -> String {
    format!("icecast:{id}")
}

// [SEC-06]
impl Secrets {
    pub fn get(&self, key: &str) -> Option<String> {
        if let Some(v) = self.memory.lock().ok()?.get(key) {
            return Some(v.clone());
        }
        keyring::Entry::new(SERVICE, key)
            .and_then(|e| e.get_password())
            .ok()
    }

    pub fn has(&self, key: &str) -> bool {
        self.get(key).is_some_and(|p| !p.is_empty())
    }

    pub fn set(&self, key: &str, value: &str) -> Stored {
        match keyring::Entry::new(SERVICE, key).and_then(|e| e.set_password(value)) {
            Ok(()) => {
                if let Ok(mut m) = self.memory.lock() {
                    m.remove(key);
                }
                Stored::Keyring
            }
            Err(e) => {
                let msg = e.to_string();
                if let Ok(mut m) = self.memory.lock() {
                    m.insert(key.to_string(), value.to_string());
                }
                if let Ok(mut l) = self.last_error.lock() {
                    *l = Some(msg.clone());
                }
                Stored::Memory(msg)
            }
        }
    }

    pub fn delete(&self, key: &str) {
        if let Ok(mut m) = self.memory.lock() {
            m.remove(key);
        }
        if let Ok(e) = keyring::Entry::new(SERVICE, key) {
            let _ = e.delete_credential();
        }
    }

    pub fn store_label(&self) -> String {
        let degraded = self.last_error.lock().ok().and_then(|l| l.clone());
        match degraded {
            Some(e) => format!("mémoire uniquement (trousseau indisponible : {e})"),
            None => {
                if cfg!(target_os = "linux") {
                    "trousseau du système (KWallet / Secret Service)".into()
                } else if cfg!(target_os = "windows") {
                    "Gestionnaire d'identifiants Windows".into()
                } else {
                    "Trousseau macOS".into()
                }
            }
        }
    }
}
