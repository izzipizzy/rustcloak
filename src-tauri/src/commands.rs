use rustcloak_core::clone::clone_profile;
use rustcloak_core::engine::{resolve, EngineInfo};
use rustcloak_core::ext::install_many;
use rustcloak_core::launch::{spawn, RunningBrowser};
use rustcloak_core::model::{NewProfile, Profile, RunStatus};
use rustcloak_core::paths;
use rustcloak_core::proxy::check;
use rustcloak_core::store::ProfileStore;
use rustcloak_core::update::{is_newer, should_check, latest_available_version, pinned_version};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub root: PathBuf,
    pub store: Mutex<ProfileStore>,
    pub engine_path: Mutex<Option<PathBuf>>,
    pub running: Mutex<HashMap<String, RunningBrowser>>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let root = paths::app_root();
        std::fs::create_dir_all(&root)?;
        std::fs::create_dir_all(paths::engines_dir(&root))?;
        let store = ProfileStore::open(&paths::db_path(&root))?;
        // Restore a previously-configured engine path so onboarding is only shown once.
        let engine_path = store.get_setting("engine_path")?.map(PathBuf::from);
        Ok(Self {
            root,
            store: Mutex::new(store),
            engine_path: Mutex::new(engine_path),
            running: Mutex::new(HashMap::new()),
        })
    }

    fn engine(&self) -> Result<EngineInfo, String> {
        let configured = self.engine_path.lock().unwrap().clone();
        resolve(&paths::engines_dir(&self.root), configured.as_deref()).map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub fn list_profiles(state: tauri::State<AppState>) -> Result<Vec<Profile>, String> {
    state
        .store
        .lock()
        .unwrap()
        .list()
        .map_err(|e| e.to_string())
}

/// Create a profile, then auto-install the global default extension list into it.
/// Lock is released before awaiting the network installs (never hold a std Mutex
/// guard across .await).
#[tauri::command]
pub async fn create_profile(
    state: tauri::State<'_, AppState>,
    new: NewProfile,
) -> Result<Profile, String> {
    let (profile, defaults) = {
        let store = state.store.lock().unwrap();
        let profile = store.create(new).map_err(|e| e.to_string())?;
        let defaults = store.default_extensions().map_err(|e| e.to_string())?;
        (profile, defaults)
    };
    if !defaults.is_empty() {
        let ext_dir = paths::extensions_dir(&state.root, &profile.id);
        let (_installed, errors) = install_many(&defaults, &ext_dir).await;
        if !errors.is_empty() {
            eprintln!("default extension install errors: {}", errors.join("; "));
        }
    }
    Ok(profile)
}

#[tauri::command]
pub fn get_default_extensions(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    state
        .store
        .lock()
        .unwrap()
        .default_extensions()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_default_extensions(
    state: tauri::State<AppState>,
    sources: Vec<String>,
) -> Result<(), String> {
    state
        .store
        .lock()
        .unwrap()
        .set_default_extensions(&sources)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_profile(state: tauri::State<AppState>, profile: Profile) -> Result<(), String> {
    state
        .store
        .lock()
        .unwrap()
        .update(&profile)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_profile(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    // Stop the browser if running, drop the DB row, then delete the on-disk
    // profile directory (userdata + extensions).
    if let Some(mut running) = state.running.lock().unwrap().remove(&id) {
        let _ = running.stop();
    }
    state.store.lock().unwrap().delete(&id).map_err(|e| e.to_string())?;
    let dir = paths::profile_dir(&state.root, &id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_engine_path(state: tauri::State<AppState>, path: String) -> Result<(), String> {
    *state.engine_path.lock().unwrap() = Some(PathBuf::from(&path));
    state
        .store
        .lock()
        .unwrap()
        .set_setting("engine_path", &path)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// True if a usable engine binary is currently resolvable (path set + exists).
#[tauri::command]
pub fn engine_configured(state: tauri::State<AppState>) -> Result<bool, String> {
    Ok(state.engine().is_ok())
}

#[tauri::command]
pub async fn launch_profile(state: tauri::State<'_, AppState>, id: String) -> Result<u16, String> {
    let engine = state.engine()?;

    // Already running? Return the existing CDP port (don't orphan the child).
    if let Some(r) = state.running.lock().unwrap().get(&id) {
        return Ok(r.cdp_port);
    }

    let profile = {
        let store = state.store.lock().unwrap();
        store.get(&id).map_err(|e| e.to_string())?.ok_or("profile not found")?
    };

    // Resolve geo for any field in Auto mode that needs the proxy's exit IP.
    let needs_auto = matches!(profile.language_mode, rustcloak_core::model::GeoMode::Auto)
        || matches!(profile.timezone_mode, rustcloak_core::model::GeoMode::Auto);
    // Resolve via the actual exit IP: through the proxy if set, else directly
    // (the machine's own public IP). This is what "Auto (by IP)" means.
    let (auto_tz, auto_locale) = if needs_auto {
        let resolved = match &profile.proxy {
            Some(proxy) => rustcloak_core::proxy::resolve_geo(proxy).await,
            None => rustcloak_core::proxy::resolve_geo_direct().await,
        };
        match resolved {
            Ok(g) => g,
            Err(e) => {
                eprintln!("geo resolve failed for {id}: {e}");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    use rustcloak_core::model::GeoMode;
    let timezone = match profile.timezone_mode {
        GeoMode::Manual => profile.timezone.clone(),
        GeoMode::Auto => auto_tz,
    };
    let locale = match profile.language_mode {
        GeoMode::Manual => profile.language.clone(),
        GeoMode::Auto => auto_locale,
    };

    let userdata = paths::userdata_dir(&state.root, &id);
    let ext_root = paths::extensions_dir(&state.root, &id);
    let mut ext_dirs = Vec::new();
    if ext_root.exists() {
        for e in std::fs::read_dir(&ext_root).map_err(|e| e.to_string())? {
            let p = e.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                ext_dirs.push(p);
            }
        }
    }

    let running = spawn(&engine, &profile, &userdata, &ext_dirs, timezone.as_deref(), locale.as_deref())
        .map_err(|e| e.to_string())?;
    let port = running.cdp_port;
    let pid = running.child.id();
    state.running.lock().unwrap().insert(id.clone(), running);

    let mut profile = profile;
    profile.status = RunStatus::Running { pid, cdp_port: port };
    state.store.lock().unwrap().update(&profile).map_err(|e| e.to_string())?;
    Ok(port)
}

#[tauri::command]
pub fn stop_profile(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    if let Some(mut running) = state.running.lock().unwrap().remove(&id) {
        let _ = running.stop();
    }
    let store = state.store.lock().unwrap();
    if let Some(mut profile) = store.get(&id).map_err(|e| e.to_string())? {
        profile.status = RunStatus::Stopped;
        store.update(&profile).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn check_proxy(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let proxy = {
        let store = state.store.lock().unwrap();
        store
            .get(&id)
            .map_err(|e| e.to_string())?
            .and_then(|p| p.proxy)
    };
    let proxy = proxy.ok_or("profile has no proxy")?;
    let status = check(&proxy).await.map_err(|e| e.to_string())?;
    let store = state.store.lock().unwrap();
    if let Some(mut p) = store.get(&id).map_err(|e| e.to_string())? {
        p.proxy_status = status;
        store.update(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Install a list of extension sources into one profile. Returns the per-source
/// error messages (empty = all succeeded).
#[tauri::command]
pub async fn add_extensions(
    state: tauri::State<'_, AppState>,
    id: String,
    sources: Vec<String>,
) -> Result<Vec<String>, String> {
    let ext_root = paths::extensions_dir(&state.root, &id);
    let (_installed, errors) = install_many(&sources, &ext_root).await;
    Ok(errors)
}

#[tauri::command]
pub fn clone_profile_cmd(
    state: tauri::State<AppState>,
    id: String,
    name: String,
    inherit_proxy: bool,
) -> Result<Profile, String> {
    let store = state.store.lock().unwrap();
    clone_profile(&store, &state.root, &id, &name, inherit_proxy).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn profile_size(state: tauri::State<AppState>, id: String) -> Result<u64, String> {
    Ok(rustcloak_core::paths::dir_size(&paths::profile_dir(&state.root, &id)))
}

/// Clear cache for ONE profile (keeps cookies & history). Refuses if it's running
/// (its cache files are locked). Returns bytes freed.
#[tauri::command]
pub fn clear_profile_cache(state: tauri::State<AppState>, id: String) -> Result<u64, String> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err("Stop the profile before clearing its cache".into());
    }
    Ok(rustcloak_core::maintenance::clear_cache(&paths::userdata_dir(&state.root, &id)))
}

/// Clear caches for all STOPPED profiles (running ones are skipped — their cache
/// files are locked). Keeps cookies & history. Returns total bytes freed.
#[tauri::command]
pub fn clear_all_caches(state: tauri::State<AppState>) -> Result<u64, String> {
    let ids: Vec<String> = state
        .store
        .lock()
        .unwrap()
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| p.id)
        .collect();
    let running: std::collections::HashSet<String> =
        state.running.lock().unwrap().keys().cloned().collect();
    let mut freed = 0u64;
    for id in ids {
        if running.contains(&id) {
            continue;
        }
        freed += rustcloak_core::maintenance::clear_cache(&paths::userdata_dir(&state.root, &id));
    }
    Ok(freed)
}

#[derive(serde::Serialize)]
pub struct UpdateAvailable {
    pub version: String,
}

/// Returns Some(update) if a newer engine version is available. Throttled to 24h
/// via `last_update_check` unless `force`.
#[tauri::command]
pub async fn check_for_update(state: tauri::State<'_, AppState>, force: bool) -> Result<Option<UpdateAvailable>, String> {
    let (last_check, installed) = {
        let store = state.store.lock().unwrap();
        (
            store.get_setting("last_update_check").map_err(|e| e.to_string())?,
            store.get_setting("engine_version").map_err(|e| e.to_string())?.unwrap_or_default(),
        )
    };
    let now = chrono::Utc::now().to_rfc3339();
    if !force && !should_check(last_check.as_deref(), &now, 24) {
        return Ok(None);
    }
    let latest = match latest_available_version().await {
        Ok(v) => v,
        Err(e) => { eprintln!("update check failed: {e}"); return Ok(None); }
    };
    {
        let store = state.store.lock().unwrap();
        store.set_setting("last_update_check", &now).map_err(|e| e.to_string())?;
    }
    if is_newer(&latest, &installed) {
        Ok(Some(UpdateAvailable { version: latest }))
    } else {
        Ok(None)
    }
}

/// Download a specific engine version, extract it, set it as the active engine.
#[tauri::command]
pub async fn download_update(state: tauri::State<'_, AppState>, version: String) -> Result<(), String> {
    let engines = paths::engines_dir(&state.root);
    let exe = rustcloak_core::update::download_engine(&engines, &version).await.map_err(|e| e.to_string())?;
    let exe_str = exe.to_string_lossy().to_string();
    *state.engine_path.lock().unwrap() = Some(exe);
    let store = state.store.lock().unwrap();
    store.set_setting("engine_path", &exe_str).map_err(|e| e.to_string())?;
    store.set_setting("engine_version", &version).map_err(|e| e.to_string())?;
    Ok(())
}

/// Onboarding one-click: download the newest available engine (falls back to the
/// pinned version if the release list can't be fetched), extract, activate it.
/// Returns the executable path.
#[tauri::command]
pub async fn download_engine(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let version = latest_available_version().await.unwrap_or_else(|_| pinned_version().to_string());
    let engines = paths::engines_dir(&state.root);
    let exe = rustcloak_core::update::download_engine(&engines, &version).await.map_err(|e| e.to_string())?;
    let exe_str = exe.to_string_lossy().to_string();
    *state.engine_path.lock().unwrap() = Some(exe);
    let store = state.store.lock().unwrap();
    store.set_setting("engine_path", &exe_str).map_err(|e| e.to_string())?;
    store.set_setting("engine_version", &version).map_err(|e| e.to_string())?;
    Ok(exe_str)
}
