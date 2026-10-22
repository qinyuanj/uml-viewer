//! Mailbox polling (#142) is a change detector: it reports what changed
//! since the last poll so the frame loop can repaint only when there is
//! something to apply. The first poll is "due" (the LOVE viewer's polling
//! clock starts empty), so mail already queued at launch is consumed.

use std::path::{Path, PathBuf};

use crate::watch;

/// The files a frame must notice.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stamps {
    pub inbox: Option<String>,
    /// The newest worker event file: its name, then its stamp.
    pub event: Option<(String, String)>,
}

fn stamp(path: &Path) -> Option<String> {
    watch::mtime(path).ok()
}

/// The newest `companion-worker-<token>.event-<n>.edn` under the state dir.
pub fn newest_event(root: &Path) -> Option<(String, String)> {
    let dir = root.join(".uml-viewer");
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(index) = name
            .strip_prefix("companion-worker-")
            .and_then(|rest| rest.split_once(".event-"))
            .and_then(|(_, n)| n.strip_suffix(".edn"))
            .and_then(|n| n.parse::<u64>().ok())
        else {
            continue;
        };
        if best.as_ref().is_none_or(|(seen, _)| index > *seen) {
            best = Some((index, entry.path()));
        }
    }
    let (_, path) = best?;
    let name = path.file_name()?.to_string_lossy().into_owned();
    Some((name, stamp(&path)?))
}

/// Read the current stamps of everything the mail loop watches.
pub fn stamps(root: &Path) -> Stamps {
    Stamps {
        inbox: stamp(&root.join(".uml-viewer/to-viewer.edn")),
        event: newest_event(root),
    }
}

/// What one poll found.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Found {
    pub mail: bool,
    pub worker: bool,
}

impl Found {
    pub fn any(self) -> bool {
        self.mail || self.worker
    }
}

/// The change detector the app holds between frames.
#[derive(Debug, Default)]
pub struct Watch {
    seen: Option<Stamps>,
}

impl Watch {
    pub fn new() -> Watch {
        Watch::default()
    }

    /// Compare the current files with the last poll. The first poll reports
    /// everything as due, so a queue written before launch is consumed.
    pub fn poll(&mut self, root: &Path) -> Found {
        let current = stamps(root);
        let Some(previous) = self.seen.replace(current) else {
            return Found {
                mail: true,
                worker: true,
            };
        };
        let now = self.seen.clone().unwrap_or_default();
        Found {
            mail: previous.inbox != now.inbox,
            worker: previous.event != now.event,
        }
    }

    /// Adopt the files as already seen. Called after this viewer rewrote the
    /// queue itself (a pop changes the stamp it is watching).
    pub fn baseline(&mut self, root: &Path) {
        self.seen = Some(stamps(root));
    }
}

/// Wake the UI when the mail files change, without a frame source: the
/// thread only stats the queue and the newest worker event every 250 ms and
/// calls `wake` on a *difference*. An idle project produces no wake at all,
/// so it cannot trigger a repaint or a draw-command rebuild (#142).
pub struct Waker {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    dirty: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Waker {
    pub fn stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Did a watched file change since the last check? Clears the flag.
    pub fn take(&self) -> bool {
        self.dirty.swap(false, std::sync::atomic::Ordering::SeqCst)
    }
}

pub fn spawn(
    root: std::path::PathBuf,
    interval: std::time::Duration,
    wake: impl Fn() + Send + 'static,
) -> Waker {
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let dirty = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = std::sync::Arc::clone(&stop);
    let pending = std::sync::Arc::clone(&dirty);
    std::thread::spawn(move || {
        let mut seen: Option<Stamps> = None;
        loop {
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            let current = stamps(&root);
            if seen.as_ref() != Some(&current) {
                // The first sweep adopts the baseline silently: startup is
                // already a frame, and the frame's own poll consumes mail.
                if seen.is_some() {
                    pending.store(true, std::sync::atomic::Ordering::SeqCst);
                    wake();
                }
                seen = Some(current);
            }
            std::thread::sleep(interval);
        }
    });
    Waker { stop, dirty }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> PathBuf {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "uml mail watch {} {}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join(".uml-viewer")).unwrap();
        dir
    }

    #[test]
    fn the_first_poll_is_due_and_an_idle_poll_reports_nothing() {
        let dir = project();
        let mut watch = Watch::new();
        assert_eq!(
            Found {
                mail: true,
                worker: true
            },
            watch.poll(&dir)
        );
        assert!(!watch.poll(&dir).any(), "an unchanged project is not news");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_queued_command_is_noticed_once_and_a_pop_is_not_news_after_the_baseline() {
        let dir = project();
        let inbox = dir.join(".uml-viewer/to-viewer.edn");
        let mut watch = Watch::new();
        watch.poll(&dir);
        std::fs::write(
            &inbox,
            "{:next-id 2 :queue [{:id 1 :op :context :context :real}]}",
        )
        .unwrap();
        assert_eq!(
            Found {
                mail: true,
                worker: false
            },
            watch.poll(&dir)
        );
        assert!(!watch.poll(&dir).any());
        // The viewer pops the head itself: it must adopt that write, or its
        // own edit would look like new mail on every poll.
        std::fs::write(&inbox, "{:next-id 2 :queue []}").unwrap();
        assert!(watch.poll(&dir).mail);
        watch.baseline(&dir);
        assert!(!watch.poll(&dir).any());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_newest_worker_event_is_noticed_and_an_older_one_ignored() {
        let dir = project();
        let state = dir.join(".uml-viewer");
        let mut watch = Watch::new();
        watch.poll(&dir);
        let base = "companion-worker-abc";
        std::fs::write(
            state.join(format!("{base}.event-1.edn")),
            "{:phase :starting}",
        )
        .unwrap();
        assert!(watch.poll(&dir).worker);
        assert!(!watch.poll(&dir).any());
        std::fs::write(state.join(format!("{base}.event-2.edn")), "{:phase :ready}").unwrap();
        assert!(watch.poll(&dir).worker);
        // Rewriting an older event file does not change the newest stamp.
        std::fs::write(
            state.join(format!("{base}.event-1.edn")),
            "{:phase :starting!}",
        )
        .unwrap();
        assert!(!watch.poll(&dir).any());
        assert_eq!(
            Some((
                "companion-worker-abc.event-2.edn".to_string(),
                watch.seen.clone().unwrap().event.unwrap().1
            )),
            newest_event(&dir)
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
