use std::collections::VecDeque;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: u64,
    pub category: String,
}

#[derive(Debug, Default)]
pub struct Queue {
    items: VecDeque<Track>,
}

impl Queue {
    pub fn push(&mut self, track: Track) {
        self.items.push_back(track);
    }

    pub fn push_next(&mut self, track: Track) {
        self.items.push_front(track);
    }

    pub fn pop(&mut self) -> Option<Track> {
        self.items.pop_front()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(title: &str) -> Track {
        Track {
            path: PathBuf::from(format!("{title}.mp3")),
            title: title.into(),
            artist: String::new(),
            album: String::new(),
            duration_ms: 0,
            category: String::new(),
        }
    }

    #[test]
    fn queue_order() {
        let mut q = Queue::default();
        q.push(t("a"));
        q.push(t("b"));
        q.push_next(t("c"));
        assert_eq!(q.pop().unwrap().title, "c");
        assert_eq!(q.pop().unwrap().title, "a");
        assert_eq!(q.len(), 1);
    }
}
