use crate::model::Profile;
use std::path::{Path, PathBuf};

/// Build the CloakBrowser command-line args for a profile launch.
/// Pure function — no IO — so it can be unit-tested exhaustively.
pub fn build_args(
    profile: &Profile,
    userdata_dir: &Path,
    cdp_port: u16,
    extension_dirs: &[PathBuf],
    timezone: Option<&str>,
    locale: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", userdata_dir.display()),
        format!("--fingerprint={}", profile.seed),
        "--fingerprint-webrtc-ip=auto".to_string(),
        format!("--remote-debugging-port={}", cdp_port),
    ];
    if let Some(proxy) = &profile.proxy {
        args.push(format!("--proxy-server={}", proxy));
    }
    if !extension_dirs.is_empty() {
        let joined = extension_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(",");
        args.push(format!("--load-extension={joined}"));
        // Required alongside --load-extension for the extensions to actually load.
        args.push(format!("--disable-extensions-except={joined}"));
    }
    if let Some(tz) = timezone {
        args.push(format!("--fingerprint-timezone={tz}"));
    }
    if let Some(l) = locale {
        // All three are needed: --fingerprint-locale + --accept-lang drive
        // navigator.language/languages; --lang sets the UI language.
        args.push(format!("--lang={l}"));
        args.push(format!("--accept-lang={l}"));
        args.push(format!("--fingerprint-locale={l}"));
    }
    args
}

use std::net::TcpListener;

/// Ask the OS for a free TCP port by binding to port 0 and reading it back.
pub fn free_port() -> std::io::Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{OsProfile, ProxyStatus, RunStatus};

    fn profile(seed: u64, proxy: Option<&str>) -> Profile {
        Profile {
            id: "i".into(), name: "n".into(), seed, os_profile: OsProfile::Mac,
            proxy: proxy.map(|s| s.to_string()), proxy_status: ProxyStatus::Unknown,
            tags: vec![], group: None, notes: String::new(),
            language_mode: crate::model::GeoMode::Auto, language: None,
            timezone_mode: crate::model::GeoMode::Auto, timezone: None,
            status: RunStatus::Stopped, created_at: "t".into(), updated_at: "t".into(),
        }
    }

    #[test]
    fn includes_seed_userdata_and_port() {
        let p = profile(99, None);
        let args = build_args(&p, Path::new("/ud"), 9222, &[], None, None);
        assert!(args.contains(&"--fingerprint=99".to_string()));
        assert!(args.contains(&"--user-data-dir=/ud".to_string()));
        assert!(args.contains(&"--remote-debugging-port=9222".to_string()));
        assert!(args.contains(&"--fingerprint-webrtc-ip=auto".to_string()));
    }

    #[test]
    fn adds_proxy_only_when_set() {
        let with = build_args(&profile(1, Some("http://h:8080")), Path::new("/ud"), 1, &[], None, None);
        assert!(with.iter().any(|a| a == "--proxy-server=http://h:8080"));
        let without = build_args(&profile(1, None), Path::new("/ud"), 1, &[], None, None);
        assert!(!without.iter().any(|a| a.starts_with("--proxy-server")));
    }

    #[test]
    fn joins_extension_dirs_with_comma() {
        let exts = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        let args = build_args(&profile(1, None), Path::new("/ud"), 1, &exts, None, None);
        assert!(args.iter().any(|a| a == "--load-extension=/a,/b"));
        assert!(args.iter().any(|a| a == "--disable-extensions-except=/a,/b"));
    }

    #[test]
    fn adds_timezone_and_locale_flags_when_set() {
        let p = profile(1, None);
        let args = build_args(&p, Path::new("/ud"), 1, &[], Some("Europe/Madrid"), Some("es-ES"));
        assert!(args.iter().any(|a| a == "--fingerprint-timezone=Europe/Madrid"));
        assert!(args.iter().any(|a| a == "--lang=es-ES"));
        assert!(args.iter().any(|a| a == "--accept-lang=es-ES"));
        assert!(args.iter().any(|a| a == "--fingerprint-locale=es-ES"));
        let none = build_args(&p, Path::new("/ud"), 1, &[], None, None);
        assert!(!none.iter().any(|a| a.starts_with("--fingerprint-timezone")));
        assert!(!none.iter().any(|a| a.starts_with("--lang")));
        assert!(!none.iter().any(|a| a.starts_with("--fingerprint-locale")));
    }

    #[test]
    fn free_port_is_nonzero() {
        assert!(super::free_port().unwrap() > 0);
    }

    #[test]
    #[ignore = "requires a real CloakBrowser binary at $RUSTCLOAK_ENGINE"]
    fn spawn_and_stop_real_binary() {
        use crate::engine::EngineInfo;
        use std::path::PathBuf;
        let engine = EngineInfo { path: PathBuf::from(std::env::var("RUSTCLOAK_ENGINE").unwrap()) };
        let p = profile(123, None);
        let ud = tempfile::tempdir().unwrap();
        let mut running = super::spawn(&engine, &p, ud.path(), &[], None, None).unwrap();
        assert!(running.cdp_port > 0);
        running.stop().unwrap();
    }
}

use crate::engine::EngineInfo;
use anyhow::Result;
use std::process::{Child, Command};

pub struct RunningBrowser {
    pub child: Child,
    pub cdp_port: u16,
}

/// Spawn the browser for a profile. Caller passes resolved engine + dirs.
pub fn spawn(
    engine: &EngineInfo,
    profile: &Profile,
    userdata_dir: &Path,
    extension_dirs: &[PathBuf],
    timezone: Option<&str>,
    locale: Option<&str>,
) -> Result<RunningBrowser> {
    std::fs::create_dir_all(userdata_dir)?;
    let port = free_port()?;
    let args = build_args(profile, userdata_dir, port, extension_dirs, timezone, locale);
    let child = Command::new(&engine.path).args(&args).spawn()?;
    Ok(RunningBrowser { child, cdp_port: port })
}

impl RunningBrowser {
    pub fn stop(&mut self) -> Result<()> {
        self.child.kill()?;
        self.child.wait()?;
        Ok(())
    }
}
