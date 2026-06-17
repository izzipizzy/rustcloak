use std::path::Path;

/// Cache subdirectories (relative to a profile's user-data-dir) that are safe to
/// delete — Chromium regenerates them and they never hold cookies/history/logins.
pub const CACHE_SUBDIRS: &[&str] = &[
    "Default/Cache",
    "Default/Code Cache",
    "Default/GPUCache",
    "Default/Media Cache",
    "Default/Service Worker/CacheStorage",
    "Default/Service Worker/ScriptCache",
    "GrShaderCache",
    "ShaderCache",
    "GraphiteDawnCache",
    "component_crx_cache",
    "extensions_crx_cache",
];

/// Delete cache directories under `userdata_dir`, keeping cookies/history/logins
/// (Cookies, History, Login Data, Local Storage, IndexedDB, etc. are untouched).
/// Returns the number of bytes freed.
pub fn clear_cache(userdata_dir: &Path) -> u64 {
    let mut freed = 0;
    for sub in CACHE_SUBDIRS {
        let p = userdata_dir.join(sub);
        if p.exists() {
            freed += crate::paths::dir_size(&p);
            let _ = std::fs::remove_dir_all(&p);
        }
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clears_cache_keeps_cookies_and_history() {
        let ud = tempfile::tempdir().unwrap();
        let root = ud.path();
        // cache dirs
        std::fs::create_dir_all(root.join("Default/Cache")).unwrap();
        std::fs::write(root.join("Default/Cache/blob"), vec![0u8; 1000]).unwrap();
        std::fs::create_dir_all(root.join("GrShaderCache")).unwrap();
        std::fs::write(root.join("GrShaderCache/x"), vec![0u8; 500]).unwrap();
        // keep these
        std::fs::create_dir_all(root.join("Default")).unwrap();
        std::fs::write(root.join("Default/Cookies"), b"secret-cookie").unwrap();
        std::fs::write(root.join("Default/History"), b"my-history").unwrap();

        let freed = clear_cache(root);
        assert_eq!(freed, 1500);
        assert!(!root.join("Default/Cache").exists());
        assert!(!root.join("GrShaderCache").exists());
        assert_eq!(std::fs::read(root.join("Default/Cookies")).unwrap(), b"secret-cookie");
        assert_eq!(std::fs::read(root.join("Default/History")).unwrap(), b"my-history");
    }
}
