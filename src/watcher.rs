use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// Watches `.git` metadata inside a repository so terminal/agent Git commands auto-refresh the UI.
pub struct RepoWatcher {
    _watcher: RecommendedWatcher,
    rx: Receiver<()>,
    last_emit: Instant,
}

impl RepoWatcher {
    pub fn start(repo_root: &Path) -> anyhow::Result<Self> {
        let git_dir = repo_root.join(".git");
        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            let Ok(event) = res else {
                return;
            };
            match event.kind {
                EventKind::Access(_) => return,
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {}
                _ => {}
            }
            let all_locks = !event.paths.is_empty()
                && event.paths.iter().all(|p| {
                    p.extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext == "lock")
                });
            if all_locks {
                return;
            }
            let _ = tx.send(());
        })?;

        if git_dir.is_dir() {
            watcher.watch(&git_dir, RecursiveMode::NonRecursive)?;
            let refs_dir = git_dir.join("refs");
            if refs_dir.is_dir() {
                let _ = watcher.watch(&refs_dir, RecursiveMode::Recursive);
            }
        } else if git_dir.exists() {
            // Worktree `.git` file case: watch its parent.
            watcher.watch(repo_root, RecursiveMode::NonRecursive)?;
        }

        Ok(Self {
            _watcher: watcher,
            rx,
            last_emit: Instant::now() - Duration::from_secs(5),
        })
    }

    /// Drains queued filesystem events and returns `true` (debounced to 350ms) when a refresh is needed.
    pub fn poll_changed(&mut self) -> bool {
        let mut any = false;
        while self.rx.try_recv().is_ok() {
            any = true;
        }
        if any && self.last_emit.elapsed() >= Duration::from_millis(350) {
            self.last_emit = Instant::now();
            return true;
        }
        false
    }
}
