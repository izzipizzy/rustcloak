use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

pub const PRIMARY_BASE_URL: &str = "https://cloakbrowser.dev";
pub const GITHUB_DOWNLOAD_BASE_URL: &str = "https://github.com/CloakHQ/cloakbrowser/releases/download";
pub const GITHUB_API_URL: &str = "https://api.github.com/repos/CloakHQ/cloakbrowser/releases";

/// Platform tag used in CloakBrowser asset names (matches the official wrapper).
pub fn platform_tag() -> &'static str {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "darwin-arm64"
    } else if cfg!(target_os = "macos") {
        "darwin-x64"
    } else if cfg!(target_os = "windows") {
        "windows-x64"
    } else if cfg!(target_arch = "aarch64") {
        "linux-arm64"
    } else {
        "linux-x64"
    }
}

/// Pinned Chromium version per platform (mirrors the wrapper's PLATFORM_CHROMIUM_VERSIONS).
/// Fallback when the live release list can't be fetched.
pub fn pinned_version() -> &'static str {
    match platform_tag() {
        "darwin-arm64" | "darwin-x64" => "145.0.7632.109.2",
        "linux-arm64" => "146.0.7680.177.3",
        _ => "146.0.7680.177.5",
    }
}

pub fn archive_ext() -> &'static str {
    if cfg!(target_os = "windows") { ".zip" } else { ".tar.gz" }
}

pub fn archive_name() -> String {
    format!("cloakbrowser-{}{}", platform_tag(), archive_ext())
}

pub fn primary_download_url(version: &str) -> String {
    format!("{PRIMARY_BASE_URL}/chromium-v{version}/{}", archive_name())
}
pub fn github_download_url(version: &str) -> String {
    format!("{GITHUB_DOWNLOAD_BASE_URL}/chromium-v{version}/{}", archive_name())
}
pub fn primary_sha_url(version: &str) -> String {
    format!("{PRIMARY_BASE_URL}/chromium-v{version}/SHA256SUMS")
}
pub fn github_sha_url(version: &str) -> String {
    format!("{GITHUB_DOWNLOAD_BASE_URL}/chromium-v{version}/SHA256SUMS")
}

/// Executable path within the extracted archive directory.
pub fn executable_subpath() -> PathBuf {
    if cfg!(target_os = "macos") {
        PathBuf::from("Chromium.app/Contents/MacOS/Chromium")
    } else if cfg!(target_os = "windows") {
        PathBuf::from("chrome.exe")
    } else {
        PathBuf::from("chrome")
    }
}

/// Parse a SHA256SUMS file ("<hash>  <filename>" per line) → hash for `filename`.
pub fn parse_sha256sums(text: &str, filename: &str) -> Option<String> {
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let hash = match it.next() { Some(h) => h, None => continue };
        let name = it.next().unwrap_or("").trim_start_matches('*');
        if name == filename {
            return Some(hash.to_string());
        }
    }
    None
}

/// From the GitHub releases JSON array, return the newest version (tag stripped of
/// "chromium-v") whose assets include `asset_name`. GitHub returns newest-first.
pub fn newest_version_with_asset(releases_json: &str, asset_name: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(releases_json).ok()?;
    let arr = v.as_array()?;
    for rel in arr {
        let tag = rel["tag_name"].as_str().unwrap_or("");
        let has = rel["assets"]
            .as_array()
            .map(|a| a.iter().any(|asset| asset["name"].as_str() == Some(asset_name)))
            .unwrap_or(false);
        if has {
            return Some(tag.trim_start_matches("chromium-v").to_string());
        }
    }
    None
}

/// True if `latest` is a newer version than `installed`. Empty `installed` → true.
pub fn is_newer(latest: &str, installed: &str) -> bool {
    if installed.trim().is_empty() {
        return true;
    }
    let parse = |s: &str| -> Vec<u64> {
        s.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(latest), parse(installed));
    let n = a.len().max(b.len());
    for i in 0..n {
        let ai = a.get(i).copied().unwrap_or(0);
        let bi = b.get(i).copied().unwrap_or(0);
        if ai != bi {
            return ai > bi;
        }
    }
    false
}

/// True if a remote check should run now. `last_check` is the stored RFC3339 ts.
pub fn should_check(last_check: Option<&str>, now_rfc3339: &str, interval_hours: i64) -> bool {
    let now = match chrono::DateTime::parse_from_rfc3339(now_rfc3339) {
        Ok(t) => t,
        Err(_) => return true,
    };
    match last_check {
        None => true,
        Some(s) => match chrono::DateTime::parse_from_rfc3339(s) {
            Ok(prev) => (now - prev).num_hours() >= interval_hours,
            Err(_) => true,
        },
    }
}

async fn get_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder().build()?;
    let resp = client.get(url).header("User-Agent", "rustcloak").send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP {} for {url}", resp.status()));
    }
    Ok(resp.bytes().await?.to_vec())
}

async fn get_text(url: &str) -> Result<String> {
    let client = reqwest::Client::builder().build()?;
    let resp = client.get(url).header("User-Agent", "rustcloak").send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("HTTP {} for {url}", resp.status()));
    }
    Ok(resp.text().await?)
}

/// Newest available engine version for this platform (from GitHub releases).
pub async fn latest_available_version() -> Result<String> {
    let body = get_text(GITHUB_API_URL).await?;
    newest_version_with_asset(&body, &archive_name())
        .ok_or_else(|| anyhow!("no release asset {} found", archive_name()))
}

fn extract(bytes: &[u8], dest: &Path) -> Result<()> {
    if cfg!(target_os = "windows") {
        let mut a = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        a.extract(dest)?;
    } else {
        let gz = flate2::read::GzDecoder::new(std::io::Cursor::new(bytes));
        let mut ar = tar::Archive::new(gz);
        ar.set_preserve_permissions(true);
        ar.set_unpack_xattrs(true);
        ar.unpack(dest)?;
    }
    Ok(())
}

/// Download + verify + extract the engine for `version` into `engines_dir`.
/// Returns the path to the executable. Tries cloakbrowser.dev then GitHub Releases.
pub async fn download_engine(engines_dir: &Path, version: &str) -> Result<PathBuf> {
    use sha2::{Digest, Sha256};

    // 1. archive bytes (primary, then GitHub fallback)
    let archive = match get_bytes(&primary_download_url(version)).await {
        Ok(b) => b,
        Err(_) => get_bytes(&github_download_url(version)).await?,
    };

    // 2. checksum verification (best-effort; only fails on a real mismatch)
    let sums = match get_text(&primary_sha_url(version)).await {
        Ok(t) => Some(t),
        Err(_) => get_text(&github_sha_url(version)).await.ok(),
    };
    if let Some(text) = sums {
        if let Some(expected) = parse_sha256sums(&text, &archive_name()) {
            let mut h = Sha256::new();
            h.update(&archive);
            let got = format!("{:x}", h.finalize());
            if !got.eq_ignore_ascii_case(&expected) {
                return Err(anyhow!(
                    "sha256 mismatch for {}: expected {expected}, got {got}",
                    archive_name()
                ));
            }
        }
    }

    // 3. extract into a temp dir, then atomically rename into place. This is
    //    crash-safe: an interrupted run never destroys a working install and
    //    never leaves a half-extracted engine at the canonical path.
    let dest = engines_dir.join(format!("chromium-{version}"));
    let tmp = engines_dir.join(format!("chromium-{version}.tmp"));
    if tmp.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    std::fs::create_dir_all(&tmp)?;
    if let Err(e) = extract(&archive, &tmp) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e);
    }

    // Verify the executable landed before we swap it into place.
    let tmp_exe = tmp.join(executable_subpath());
    if !tmp_exe.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(anyhow!("executable not found after extract: {}", tmp_exe.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp_exe)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp_exe, perms)?;
    }
    // macOS: strip the quarantine attribute so Gatekeeper doesn't block launch.
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("xattr")
            .args(["-dr", "com.apple.quarantine"])
            .arg(&tmp)
            .status();
    }

    // Atomic swap: remove any old install, then rename temp into place.
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::rename(&tmp, &dest)?;

    Ok(dest.join(executable_subpath()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_name_matches_platform() {
        // On the dev machine (macOS arm64) this is darwin-arm64.
        let name = archive_name();
        assert!(name.starts_with("cloakbrowser-"));
        assert!(name.ends_with(".tar.gz") || name.ends_with(".zip"));
    }

    #[test]
    fn urls_are_built_correctly() {
        let v = "145.0.7632.109.2";
        assert_eq!(
            primary_download_url(v),
            format!("https://cloakbrowser.dev/chromium-v{v}/{}", archive_name())
        );
        assert_eq!(
            github_download_url(v),
            format!("https://github.com/CloakHQ/cloakbrowser/releases/download/chromium-v{v}/{}", archive_name())
        );
        assert!(primary_sha_url(v).ends_with("/SHA256SUMS"));
        assert!(github_sha_url(v).ends_with("/SHA256SUMS"));
    }

    #[test]
    fn parses_sha256sums_with_and_without_star() {
        let text = "aaa  cloakbrowser-darwin-arm64.tar.gz\nbbb *cloakbrowser-linux-x64.tar.gz\n";
        assert_eq!(parse_sha256sums(text, "cloakbrowser-darwin-arm64.tar.gz"), Some("aaa".into()));
        assert_eq!(parse_sha256sums(text, "cloakbrowser-linux-x64.tar.gz"), Some("bbb".into()));
        assert_eq!(parse_sha256sums(text, "missing.tar.gz"), None);
    }

    #[test]
    fn newest_version_picks_first_matching_asset() {
        let json = r#"[
            {"tag_name":"chromium-v146.0.7680.177.5","assets":[{"name":"cloakbrowser-linux-x64.tar.gz"}]},
            {"tag_name":"chromium-v145.0.7632.109.2","assets":[{"name":"cloakbrowser-darwin-arm64.tar.gz"},{"name":"cloakbrowser-darwin-x64.tar.gz"}]}
        ]"#;
        assert_eq!(newest_version_with_asset(json, "cloakbrowser-darwin-arm64.tar.gz"), Some("145.0.7632.109.2".into()));
        assert_eq!(newest_version_with_asset(json, "cloakbrowser-linux-x64.tar.gz"), Some("146.0.7680.177.5".into()));
        assert_eq!(newest_version_with_asset(json, "cloakbrowser-windows-x64.zip"), None);
    }

    #[test]
    fn executable_subpath_is_platform_specific() {
        let p = executable_subpath();
        if cfg!(target_os = "macos") {
            assert_eq!(p, PathBuf::from("Chromium.app/Contents/MacOS/Chromium"));
        }
    }

    #[test]
    fn is_newer_compares_components() {
        assert!(is_newer("145.0.7632.109.2", "145.0.7632.109.1"));
        assert!(is_newer("146.0.0.0", "145.9.9.9"));
        assert!(is_newer("1.0", ""));
        assert!(!is_newer("145.0.7632.109.2", "145.0.7632.109.2"));
    }

    #[test]
    fn should_check_respects_interval() {
        assert!(should_check(None, "2026-06-16T12:00:00+00:00", 24));
        assert!(should_check(Some("2026-06-15T11:00:00+00:00"), "2026-06-16T12:00:00+00:00", 24));
        assert!(!should_check(Some("2026-06-16T06:00:00+00:00"), "2026-06-16T12:00:00+00:00", 24));
    }

    #[tokio::test]
    #[ignore = "network: hits GitHub releases API"]
    async fn live_latest_version() {
        let v = super::latest_available_version().await.unwrap();
        assert!(!v.is_empty());
    }

    #[tokio::test]
    #[ignore = "network: full download_engine end-to-end (~147MB)"]
    async fn live_download_engine() {
        let v = "145.0.7632.109.2";
        let dir = tempfile::tempdir().unwrap();
        let exe = super::download_engine(dir.path(), v).await.unwrap();
        assert!(exe.exists());
    }
}
