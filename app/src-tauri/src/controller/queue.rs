use std::cell::Cell;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};

use super::Controller;
use crate::journal;
use crate::playlist::{load_queue, make_entry, save_queue, Entry};

// [CTL-155]
const SAVE_AFTER: Duration = Duration::from_secs(2);

// [CTL-156]
struct Snapshot {
    entries: Vec<Entry>,
    decks: Vec<PathBuf>,
}

// [CTL-157]
#[derive(Default)]
pub struct QueueState {
    path: Option<PathBuf>,
    tx: Option<Sender<Snapshot>>,
    dirty: Cell<Option<Instant>>,
}

impl QueueState {
    pub fn mark(&self) {
        if self.path.is_some() && self.dirty.get().is_none() {
            self.dirty.set(Some(Instant::now()));
        }
    }
}

// [CTL-158]
fn saver(path: PathBuf, rx: Receiver<Snapshot>) {
    while let Ok(mut snap) = rx.recv() {
        while let Ok(newer) = rx.try_recv() {
            snap = newer;
        }
        if let Err(e) = save_queue(&path, &snap.entries, &snap.decks) {
            journal::warn(format!(
                "sauvegarde de la playlist {} : {e}",
                path.display()
            ));
        }
    }
}

impl Controller {
    // [CTL-159]
    pub(super) fn restore_queue(&mut self, path: PathBuf) {
        if path.exists() {
            match load_queue(&path) {
                Ok((entries, decks)) => {
                    let mut items: Vec<Entry> = decks
                        .iter()
                        .filter(|p| p.exists())
                        .map(|p| make_entry(p, &self.categories))
                        .collect();
                    items.extend(entries);
                    let n = items.len();
                    self.playlist.insert(items, None);
                    self.playlist.reclassify(&self.categories);
                    if n > 0 {
                        journal::info(format!("playlist reprise : {n} titre(s)"));
                    }
                }
                Err(e) => journal::warn(format!("playlist {} illisible : {e}", path.display())),
            }
        }
        let (tx, rx) = unbounded();
        let target = path.clone();
        let spawned = thread::Builder::new()
            .name("blast-queue".into())
            .spawn(move || saver(target, rx));
        match spawned {
            Ok(_) => self.queue.tx = Some(tx),
            Err(e) => journal::warn(format!("sauvegarde de la playlist : {e}")),
        }
        self.queue.path = Some(path);
    }

    // [CTL-160]
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            entries: self.playlist.entries().to_vec(),
            decks: self
                .decks
                .iter()
                .filter(|d| !d.meta.finished)
                .filter_map(|d| d.meta.path.clone())
                .collect(),
        }
    }

    // [CTL-161]
    pub(super) fn save_queue_if_due(&mut self) {
        let due = self
            .queue
            .dirty
            .get()
            .is_some_and(|t| t.elapsed() >= SAVE_AFTER);
        if !due {
            return;
        }
        self.queue.dirty.set(None);
        let snap = self.snapshot();
        if let Some(tx) = &self.queue.tx {
            let _ = tx.send(snap);
        }
    }

    // [CTL-162]
    pub(super) fn flush_queue(&mut self) {
        if self.queue.dirty.take().is_none() {
            return;
        }
        let Some(path) = self.queue.path.clone() else {
            return;
        };
        let snap = self.snapshot();
        if let Err(e) = save_queue(&path, &snap.entries, &snap.decks) {
            journal::warn(format!(
                "sauvegarde de la playlist {} : {e}",
                path.display()
            ));
        }
    }
}
