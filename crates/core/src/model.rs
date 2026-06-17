use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OsProfile { Mac, Windows }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GeoMode { Auto, Manual }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProxyStatus {
    Unknown,
    Ok { ip: String, country: String, latency_ms: u32 },
    Dead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum RunStatus {
    Stopped,
    Running { pid: u32, cdp_port: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub os_profile: OsProfile,
    pub proxy: Option<String>,
    pub proxy_status: ProxyStatus,
    pub tags: Vec<String>,
    pub group: Option<String>,
    pub notes: String,
    pub language_mode: GeoMode,
    pub language: Option<String>,
    pub timezone_mode: GeoMode,
    pub timezone: Option<String>,
    pub status: RunStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// Fields supplied by the UI when creating a profile; the store fills the rest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProfile {
    pub name: String,
    pub os_profile: OsProfile,
    pub proxy: Option<String>,
    pub tags: Vec<String>,
    pub group: Option<String>,
    pub notes: String,
    pub language_mode: GeoMode,
    pub language: Option<String>,
    pub timezone_mode: GeoMode,
    pub timezone: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_round_trips_through_json() {
        let p = Profile {
            id: "id1".into(), name: "Acc 1".into(), seed: 42,
            os_profile: OsProfile::Mac, proxy: Some("http://h:1".into()),
            proxy_status: ProxyStatus::Ok { ip: "1.2.3.4".into(), country: "ES".into(), latency_ms: 12 },
            tags: vec!["ads".into()], group: Some("g1".into()), notes: "n".into(),
            language_mode: GeoMode::Auto, language: None,
            timezone_mode: GeoMode::Manual, timezone: Some("Europe/Madrid".into()),
            status: RunStatus::Stopped, created_at: "t".into(), updated_at: "t".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
