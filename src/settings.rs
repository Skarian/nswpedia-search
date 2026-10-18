//! Non-secret preferences stored as JSON in the OS config directory. The
//! Real-Debrid API key is kept in the OS credential store, never here.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// `None` means the user's Downloads folder.
    #[serde(default)]
    pub download_dir: Option<PathBuf>,
}

impl Settings {
    /// Missing or unreadable settings fall back to defaults.
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&temp, path).context("Could not save settings")
    }

    pub fn download_dir(&self) -> PathBuf {
        self.download_dir
            .clone()
            .unwrap_or_else(default_download_dir)
    }
}

pub fn default_download_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|dirs| {
            dirs.download_dir()
                .map(Path::to_path_buf)
                .or_else(|| Some(dirs.home_dir().join("Downloads")))
        })
        .unwrap_or_else(|| PathBuf::from("Downloads"))
}

/// Validates a folder typed in settings. Empty means "use the default".
pub fn parse_download_dir(input: &str) -> Result<Option<PathBuf>> {
    let input = input.trim().trim_matches('"');
    if input.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(input);
    ensure!(
        path.is_absolute(),
        "Download folder must be a full path, like C:\\Users\\you\\Downloads"
    );
    ensure!(
        !path.exists() || path.is_dir(),
        "Download folder points to a file, not a folder"
    );
    Ok((path != default_download_dir()).then_some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_corrupt_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg").join("settings.json");
        assert_eq!(Settings::load(&path), Settings::default());
        let settings = Settings {
            download_dir: Some(dir.path().join("dl")),
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        assert_eq!(Settings::default().download_dir(), default_download_dir());
    }

    #[test]
    fn download_dir_input_is_validated() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(parse_download_dir("  ").unwrap(), None);
        assert!(parse_download_dir("relative\\folder").is_err());
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "x").unwrap();
        assert!(parse_download_dir(file.to_str().unwrap()).is_err());
        let quoted = format!("\"{}\"", dir.path().display());
        assert_eq!(
            parse_download_dir(&quoted).unwrap(),
            Some(dir.path().to_path_buf())
        );
    }
}
