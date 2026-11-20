use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_RECENT_REPOS: usize = 16;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub recent_repos: Vec<PathBuf>,
    #[serde(default)]
    pub last_repo: Option<PathBuf>,
}

impl AppConfig {
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("git-out").join("config.json"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        let Ok(bytes) = fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&bytes).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, json);
        }
    }

    pub fn record_repo(&mut self, repo_root: &Path) {
        let canonical = fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
        self.recent_repos.retain(|p| p != &canonical);
        self.recent_repos.insert(0, canonical.clone());
        self.recent_repos.truncate(MAX_RECENT_REPOS);
        self.last_repo = Some(canonical);
        self.save();
    }
}

/// Scans immediate subdirectories of `~/` (and `~/Projects` if present) for `.git` directories
/// so the Open Repository dialog offers quick one-click repository switching.
pub fn discover_home_git_repos() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Some(home) = dirs::home_dir() else {
        return found;
    };

    let mut search_roots = vec![home.clone()];
    let projects = home.join("Projects");
    if projects.is_dir() {
        search_roots.push(projects);
    }

    for root in search_roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.starts_with('.')
            {
                continue;
            }
            if path.join(".git").exists() && !found.contains(&path) {
                found.push(path);
            }
        }
    }

    found.sort_by(|a, b| {
        let an = a.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let bn = b.file_name().and_then(|s| s.to_str()).unwrap_or("");
        an.to_lowercase().cmp(&bn.to_lowercase())
    });
    found
}
