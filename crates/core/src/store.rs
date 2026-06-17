use crate::model::{NewProfile, Profile, ProxyStatus, RunStatus};
use crate::seed::gen_seed;
use anyhow::Result;
use rusqlite::Connection;

pub struct ProfileStore {
    conn: Connection,
}

impl ProfileStore {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                seed TEXT NOT NULL,
                os_profile TEXT NOT NULL,
                proxy TEXT,
                proxy_status TEXT NOT NULL,
                tags TEXT NOT NULL,
                group_name TEXT,
                notes TEXT NOT NULL,
                status TEXT NOT NULL,
                language_mode TEXT,
                language TEXT,
                timezone_mode TEXT,
                timezone TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        for stmt in [
            "ALTER TABLE profiles ADD COLUMN language_mode TEXT",
            "ALTER TABLE profiles ADD COLUMN language TEXT",
            "ALTER TABLE profiles ADD COLUMN timezone_mode TEXT",
            "ALTER TABLE profiles ADD COLUMN timezone TEXT",
        ] {
            let _ = self.conn.execute(stmt, []);
        }
        Ok(())
    }

    /// Read a raw setting value by key.
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key=?1")?;
        let mut rows = stmt.query([key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Upsert a raw setting value by key.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key,value) VALUES (?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=?2",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    /// The global default extension sources (URLs / Web Store ids) auto-installed
    /// into every newly created profile. Stored as a JSON array of strings.
    pub fn default_extensions(&self) -> Result<Vec<String>> {
        match self.get_setting("default_extensions")? {
            Some(json) => Ok(serde_json::from_str(&json)?),
            None => Ok(vec![]),
        }
    }

    pub fn set_default_extensions(&self, sources: &[String]) -> Result<()> {
        self.set_setting("default_extensions", &serde_json::to_string(sources)?)
    }

    pub fn create(&self, new: NewProfile) -> Result<Profile> {
        let now = chrono::Utc::now().to_rfc3339();
        let p = Profile {
            id: uuid::Uuid::new_v4().to_string(),
            name: new.name,
            seed: gen_seed(),
            os_profile: new.os_profile,
            proxy: new.proxy,
            proxy_status: ProxyStatus::Unknown,
            tags: new.tags,
            group: new.group,
            notes: new.notes,
            language_mode: new.language_mode,
            language: new.language,
            timezone_mode: new.timezone_mode,
            timezone: new.timezone,
            status: RunStatus::Stopped,
            created_at: now.clone(),
            updated_at: now,
        };
        self.insert(&p)?;
        Ok(p)
    }

    pub fn insert(&self, p: &Profile) -> Result<()> {
        self.conn.execute(
            "INSERT INTO profiles
             (id,name,seed,os_profile,proxy,proxy_status,tags,group_name,notes,status,language_mode,language,timezone_mode,timezone,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
            rusqlite::params![
                p.id, p.name, p.seed.to_string(),
                serde_json::to_string(&p.os_profile)?,
                p.proxy,
                serde_json::to_string(&p.proxy_status)?,
                serde_json::to_string(&p.tags)?,
                p.group, p.notes,
                serde_json::to_string(&p.status)?,
                serde_json::to_string(&p.language_mode)?, p.language,
                serde_json::to_string(&p.timezone_mode)?, p.timezone,
                p.created_at, p.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, p: &Profile) -> Result<()> {
        self.conn.execute(
            "UPDATE profiles SET name=?2,seed=?3,os_profile=?4,proxy=?5,proxy_status=?6,
             tags=?7,group_name=?8,notes=?9,status=?10,language_mode=?11,language=?12,
             timezone_mode=?13,timezone=?14,updated_at=?15 WHERE id=?1",
            rusqlite::params![
                p.id, p.name, p.seed.to_string(),
                serde_json::to_string(&p.os_profile)?,
                p.proxy,
                serde_json::to_string(&p.proxy_status)?,
                serde_json::to_string(&p.tags)?,
                p.group, p.notes,
                serde_json::to_string(&p.status)?,
                serde_json::to_string(&p.language_mode)?, p.language,
                serde_json::to_string(&p.timezone_mode)?, p.timezone,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM profiles WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<Profile>> {
        let mut stmt = self.conn.prepare("SELECT * FROM profiles WHERE id=?1")?;
        let mut rows = stmt.query([id])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_profile(row)?)),
            None => Ok(None),
        }
    }

    pub fn list(&self) -> Result<Vec<Profile>> {
        let mut stmt = self.conn.prepare("SELECT * FROM profiles ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |row| Ok(row_to_profile(row)))?;
        let mut out = Vec::new();
        for r in rows { out.push(r??); }
        Ok(out)
    }
}

fn row_to_profile(row: &rusqlite::Row) -> Result<Profile> {
    let seed_str: String = row.get("seed")?;
    Ok(Profile {
        id: row.get("id")?,
        name: row.get("name")?,
        seed: seed_str.parse().unwrap_or(0),
        os_profile: serde_json::from_str(&row.get::<_, String>("os_profile")?)?,
        proxy: row.get("proxy")?,
        proxy_status: serde_json::from_str(&row.get::<_, String>("proxy_status")?)?,
        tags: serde_json::from_str(&row.get::<_, String>("tags")?)?,
        group: row.get("group_name")?,
        notes: row.get("notes")?,
        language_mode: match row.get::<_, Option<String>>("language_mode")? {
            Some(j) => serde_json::from_str(&j).unwrap_or(crate::model::GeoMode::Auto),
            None => crate::model::GeoMode::Auto,
        },
        language: row.get("language")?,
        timezone_mode: match row.get::<_, Option<String>>("timezone_mode")? {
            Some(j) => serde_json::from_str(&j).unwrap_or(crate::model::GeoMode::Auto),
            None => crate::model::GeoMode::Auto,
        },
        timezone: row.get("timezone")?,
        status: serde_json::from_str(&row.get::<_, String>("status")?)?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{GeoMode, OsProfile};

    fn sample() -> NewProfile {
        NewProfile { name: "Acc".into(), os_profile: OsProfile::Mac, proxy: None,
            tags: vec![], group: None, notes: String::new(),
            language_mode: GeoMode::Auto, language: None,
            timezone_mode: GeoMode::Auto, timezone: None }
    }

    #[test]
    fn create_then_get_returns_same_profile() {
        let store = ProfileStore::open_in_memory().unwrap();
        let p = store.create(sample()).unwrap();
        let got = store.get(&p.id).unwrap().unwrap();
        assert_eq!(got, p);
    }

    #[test]
    fn list_returns_all_created() {
        let store = ProfileStore::open_in_memory().unwrap();
        store.create(sample()).unwrap();
        store.create(sample()).unwrap();
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn delete_removes_profile() {
        let store = ProfileStore::open_in_memory().unwrap();
        let p = store.create(sample()).unwrap();
        store.delete(&p.id).unwrap();
        assert!(store.get(&p.id).unwrap().is_none());
    }

    #[test]
    fn geo_fields_round_trip() {
        let store = ProfileStore::open_in_memory().unwrap();
        let mut p = store.create(sample()).unwrap();
        assert_eq!(p.language_mode, GeoMode::Auto);
        assert_eq!(p.timezone_mode, GeoMode::Auto);
        p.language_mode = GeoMode::Manual;
        p.language = Some("es-ES".into());
        p.timezone_mode = GeoMode::Manual;
        p.timezone = Some("Europe/Madrid".into());
        store.update(&p).unwrap();
        let got = store.get(&p.id).unwrap().unwrap();
        assert_eq!(got.language, Some("es-ES".into()));
        assert_eq!(got.timezone, Some("Europe/Madrid".into()));
        assert_eq!(got.language_mode, GeoMode::Manual);
    }

    #[test]
    fn default_extensions_round_trip() {
        let store = ProfileStore::open_in_memory().unwrap();
        assert_eq!(store.default_extensions().unwrap(), Vec::<String>::new());
        let list = vec!["cjpalhdlnbpafiamejdnhcphjbkeiagm".to_string(), "https://x.com/e.crx".to_string()];
        store.set_default_extensions(&list).unwrap();
        assert_eq!(store.default_extensions().unwrap(), list);
        store.set_default_extensions(&[]).unwrap();
        assert_eq!(store.default_extensions().unwrap(), Vec::<String>::new());
    }
}
