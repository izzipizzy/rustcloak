use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineInfo {
    pub path: PathBuf,
}

/// Resolve the CloakBrowser binary: prefer an explicitly configured path,
/// otherwise look for a binary inside `engines_dir`. We never bundle the binary
/// (license: no redistribution) — it is downloaded or pointed-to by the user.
pub fn resolve(engines_dir: &Path, configured: Option<&Path>) -> Result<EngineInfo> {
    if let Some(p) = configured {
        if p.exists() {
            return Ok(EngineInfo { path: p.to_path_buf() });
        }
        return Err(anyhow!("configured engine path does not exist: {}", p.display()));
    }
    // Look for any file directly inside engines_dir.
    if engines_dir.exists() {
        for entry in std::fs::read_dir(engines_dir)? {
            let path = entry?.path();
            if path.is_file() {
                return Ok(EngineInfo { path });
            }
        }
    }
    Err(anyhow!("no CloakBrowser binary found; configure a path or download one"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn configured_path_wins_when_it_exists() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("cloak");
        std::fs::write(&bin, b"x").unwrap();
        let info = resolve(dir.path(), Some(&bin)).unwrap();
        assert_eq!(info.path, bin);
    }

    #[test]
    fn errors_when_nothing_found() {
        let dir = tempdir().unwrap();
        assert!(resolve(dir.path(), None).is_err());
    }

    #[test]
    fn finds_binary_in_engines_dir() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("cloakbrowser");
        std::fs::write(&bin, b"x").unwrap();
        let info = resolve(dir.path(), None).unwrap();
        assert_eq!(info.path, bin);
    }
}
