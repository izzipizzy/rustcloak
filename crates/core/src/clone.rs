use crate::model::{NewProfile, Profile};
use crate::store::ProfileStore;
use anyhow::Result;
use std::path::Path;

/// Recursively copy a directory tree. Symlinks and other special files are
/// SKIPPED — Chromium leaves dangling symlinks like `SingletonLock` /
/// `SingletonCookie` (their targets don't exist), and `std::fs::copy` follows
/// links, so copying them would error out the whole clone. They're per-session
/// lock markers and must not be cloned anyway.
pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if ty.is_file() {
            std::fs::copy(entry.path(), &target)?;
        }
        // else: symlink / socket / fifo — skip.
    }
    Ok(())
}

/// Clone a profile: create a new profile row with a FRESH seed, then copy the
/// source userdata + extensions into the new profile's dirs. Fingerprint is
/// NOT inherited (new seed); cookies/storage ARE inherited (copied userdata).
pub fn clone_profile(
    store: &ProfileStore,
    root: &Path,
    source_id: &str,
    new_name: &str,
    inherit_proxy: bool,
) -> Result<Profile> {
    let source = store
        .get(source_id)?
        .ok_or_else(|| anyhow::anyhow!("source profile not found"))?;

    let new = NewProfile {
        name: new_name.to_string(),
        os_profile: source.os_profile,
        proxy: if inherit_proxy { source.proxy.clone() } else { None },
        tags: source.tags.clone(),
        group: source.group.clone(),
        notes: source.notes.clone(),
        language_mode: source.language_mode,
        language: source.language.clone(),
        timezone_mode: source.timezone_mode,
        timezone: source.timezone.clone(),
    };
    // store.create() assigns a brand-new uuid AND a fresh seed.
    let created = store.create(new)?;
    debug_assert_ne!(created.seed, source.seed);

    // Copy data; on any failure, roll back the just-created row + dir so a failed
    // clone never lingers as a phantom profile.
    let copy = || -> Result<()> {
        let src_ud = crate::paths::userdata_dir(root, &source.id);
        let dst_ud = crate::paths::userdata_dir(root, &created.id);
        if src_ud.exists() {
            copy_dir_all(&src_ud, &dst_ud)?;
            // Don't carry the source's (large, volatile) cache into the clone.
            crate::maintenance::clear_cache(&dst_ud);
        }
        let src_ext = crate::paths::extensions_dir(root, &source.id);
        let dst_ext = crate::paths::extensions_dir(root, &created.id);
        if src_ext.exists() {
            copy_dir_all(&src_ext, &dst_ext)?;
        }
        Ok(())
    };
    if let Err(e) = copy() {
        let _ = std::fs::remove_dir_all(crate::paths::profile_dir(root, &created.id));
        let _ = store.delete(&created.id);
        return Err(e);
    }

    // `created` already carries the fresh seed from store.create().
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GeoMode, OsProfile};

    #[test]
    fn copy_dir_all_copies_nested_files() {
        let src = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(src.path().join("a")).unwrap();
        std::fs::write(src.path().join("a/f.txt"), b"hi").unwrap();
        let dst = tempfile::tempdir().unwrap();
        copy_dir_all(src.path(), &dst.path().join("out")).unwrap();
        assert_eq!(std::fs::read(dst.path().join("out/a/f.txt")).unwrap(), b"hi");
    }

    #[test]
    fn clone_keeps_userdata_but_changes_seed() {
        let root = tempfile::tempdir().unwrap();
        // File-backed db under root so paths line up.
        let store = ProfileStore::open(&crate::paths::db_path(root.path())).unwrap();

        let src = store.create(NewProfile {
            name: "src".into(), os_profile: OsProfile::Mac, proxy: Some("http://h:1".into()),
            tags: vec!["t".into()], group: None, notes: String::new(),
            language_mode: GeoMode::Auto, language: None,
            timezone_mode: GeoMode::Auto, timezone: None,
        }).unwrap();

        // seed the source userdata with a marker file
        let ud = crate::paths::userdata_dir(root.path(), &src.id);
        std::fs::create_dir_all(&ud).unwrap();
        std::fs::write(ud.join("cookie.txt"), b"secret").unwrap();

        let cloned = super::clone_profile(&store, root.path(), &src.id, "copy", false).unwrap();

        assert_ne!(cloned.seed, src.seed, "seed must be fresh");
        assert_eq!(cloned.proxy, None, "proxy not inherited when inherit_proxy=false");
        let cloned_cookie = crate::paths::userdata_dir(root.path(), &cloned.id).join("cookie.txt");
        assert_eq!(std::fs::read(cloned_cookie).unwrap(), b"secret", "userdata must be copied");
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_all_skips_dangling_symlinks() {
        // Reproduces the clone bug: Chromium's SingletonLock is a symlink whose
        // target does not exist; std::fs::copy on it used to error the whole copy.
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("real.txt"), b"hi").unwrap();
        std::os::unix::fs::symlink("does-not-exist", src.path().join("SingletonLock")).unwrap();
        let dst = tempfile::tempdir().unwrap();
        let out = dst.path().join("out");
        copy_dir_all(src.path(), &out).unwrap(); // must NOT error
        assert_eq!(std::fs::read(out.join("real.txt")).unwrap(), b"hi");
        assert!(!out.join("SingletonLock").exists(), "symlink must be skipped");
    }

    #[test]
    #[ignore = "repro: clones the real _base_ profile from ~/code/rustcloak/data, then cleans up"]
    fn repro_clone_real_base() {
        let home = std::env::var("HOME").unwrap();
        let root = std::path::PathBuf::from(format!("{home}/code/rustcloak/data"));
        let store = ProfileStore::open(&crate::paths::db_path(&root)).unwrap();
        let list = store.list().unwrap();
        let src = list.iter().find(|p| p.name == "_base_").expect("_base_ exists");
        let res = super::clone_profile(&store, &root, &src.id, "REPRO_CLONE_TMP", false);
        match &res {
            Ok(p) => eprintln!("clone OK -> {}", p.id),
            Err(e) => eprintln!("clone ERR: {e:?}"),
        }
        if let Ok(p) = &res {
            let _ = std::fs::remove_dir_all(crate::paths::profile_dir(&root, &p.id));
            let _ = store.delete(&p.id);
        }
        res.unwrap();
    }
}
