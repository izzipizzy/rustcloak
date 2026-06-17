use std::path::{Path, PathBuf};

/// Root app data dir (holds profiles/, engines/, db.sqlite).
///
/// Resolution order:
/// 1. `RUSTCLOAK_DATA_DIR` env var, if set — explicit override (used by packaged
///    builds or to point at a synced location).
/// 2. Dev builds (`debug_assertions`): `<repo>/data`, so `npm run tauri dev`
///    keeps everything inside the project tree for easy syncing.
/// 3. Release builds: the OS data dir (`~/Library/Application Support/rustcloak`).
pub fn app_root() -> PathBuf {
    if let Ok(dir) = std::env::var("RUSTCLOAK_DATA_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }
    #[cfg(debug_assertions)]
    {
        // crates/core/../../data == <repo root>/data
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    }
    #[cfg(not(debug_assertions))]
    {
        dirs::data_dir().expect("no data dir").join("rustcloak")
    }
}

pub fn db_path(root: &Path) -> PathBuf { root.join("db.sqlite") }
pub fn engines_dir(root: &Path) -> PathBuf { root.join("engines") }
pub fn profiles_dir(root: &Path) -> PathBuf { root.join("profiles") }
pub fn profile_dir(root: &Path, id: &str) -> PathBuf { profiles_dir(root).join(id) }
pub fn userdata_dir(root: &Path, id: &str) -> PathBuf { profile_dir(root, id).join("userdata") }
pub fn extensions_dir(root: &Path, id: &str) -> PathBuf { profile_dir(root, id).join("extensions") }

/// Total size in bytes of all regular files under `path` (recursive).
/// Missing/unreadable paths contribute 0. Symlinks are not followed.
pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => total += dir_size(&entry.path()),
                Ok(ft) if ft.is_file() => {
                    if let Ok(m) = entry.metadata() {
                        total += m.len();
                    }
                }
                _ => {}
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn layout_is_nested_under_root() {
        let root = Path::new("/tmp/rc");
        assert_eq!(db_path(root), Path::new("/tmp/rc/db.sqlite"));
        assert_eq!(userdata_dir(root, "abc"), Path::new("/tmp/rc/profiles/abc/userdata"));
        assert_eq!(extensions_dir(root, "abc"), Path::new("/tmp/rc/profiles/abc/extensions"));
        assert_eq!(engines_dir(root), Path::new("/tmp/rc/engines"));
    }

    #[test]
    fn dir_size_sums_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/f1.txt"), b"12345").unwrap();   // 5
        std::fs::write(dir.path().join("a/b/f2.txt"), b"123").unwrap();   // 3
        assert_eq!(super::dir_size(dir.path()), 8);
        assert_eq!(super::dir_size(&dir.path().join("missing")), 0);
    }
}
