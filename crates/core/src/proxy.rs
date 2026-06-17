use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyParts {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub pass: Option<String>,
}

/// Parse `scheme://[user:pass@]host:port`.
pub fn parse(s: &str) -> Result<ProxyParts> {
    let (scheme, rest) = s.split_once("://").ok_or_else(|| anyhow!("missing scheme"))?;
    let (creds, hostport) = match rest.rsplit_once('@') {
        Some((c, hp)) => (Some(c), hp),
        None => (None, rest),
    };
    let (host, port_str) = hostport.rsplit_once(':').ok_or_else(|| anyhow!("missing port"))?;
    let port: u16 = port_str.parse().map_err(|_| anyhow!("bad port"))?;
    let (user, pass) = match creds {
        Some(c) => match c.split_once(':') {
            Some((u, p)) => (Some(u.to_string()), Some(p.to_string())),
            None => (Some(c.to_string()), None),
        },
        None => (None, None),
    };
    Ok(ProxyParts {
        scheme: scheme.to_string(),
        host: host.to_string(),
        port,
        user,
        pass,
    })
}

use crate::model::ProxyStatus;

/// Parse an ip-api.com JSON response into a ProxyStatus::Ok.
/// Response shape: {"status":"success","query":"1.2.3.4","countryCode":"ES"}
pub fn parse_ipapi(json: &str, latency_ms: u32) -> Result<ProxyStatus> {
    let v: serde_json::Value = serde_json::from_str(json)?;
    if v["status"] != "success" {
        return Ok(ProxyStatus::Dead);
    }
    let ip = v["query"].as_str().ok_or_else(|| anyhow!("no query"))?.to_string();
    let country = v["countryCode"].as_str().unwrap_or("").to_string();
    Ok(ProxyStatus::Ok { ip, country, latency_ms })
}

/// Pure: parse an ip-api.com JSON response into (timezone, locale).
/// timezone is taken directly from ip-api; locale is derived from countryCode.
pub fn parse_ipapi_geo(json: &str) -> (Option<String>, Option<String>) {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return (None, None),
    };
    if v["status"] != "success" {
        return (None, None);
    }
    let tz = v["timezone"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string());
    let locale = v["countryCode"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|c| crate::geo::locale_for_country(c));
    (tz, locale)
}

/// Resolve (timezone, locale) for a proxy's exit IP via ip-api (through the proxy).
pub async fn resolve_geo(proxy: &str) -> anyhow::Result<(Option<String>, Option<String>)> {
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(proxy)?)
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let body = client.get("http://ip-api.com/json").send().await?.text().await?;
    Ok(parse_ipapi_geo(&body))
}

/// Resolve (timezone, locale) for the machine's own exit IP via ip-api (no proxy).
/// Used for profiles in Auto mode that have no proxy configured.
pub async fn resolve_geo_direct() -> anyhow::Result<(Option<String>, Option<String>)> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let body = client.get("http://ip-api.com/json").send().await?.text().await?;
    Ok(parse_ipapi_geo(&body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_proxy_with_auth() {
        let p = parse("http://bob:secret@1.2.3.4:8080").unwrap();
        assert_eq!(p, ProxyParts {
            scheme: "http".into(), host: "1.2.3.4".into(), port: 8080,
            user: Some("bob".into()), pass: Some("secret".into()),
        });
    }

    #[test]
    fn parses_proxy_without_auth() {
        let p = parse("socks5://1.2.3.4:1080").unwrap();
        assert_eq!(p.user, None);
        assert_eq!(p.port, 1080);
        assert_eq!(p.scheme, "socks5");
    }

    #[test]
    fn rejects_missing_port() {
        assert!(parse("http://1.2.3.4").is_err());
    }

    #[test]
    fn parses_successful_ipapi_response() {
        let json = r#"{"status":"success","query":"5.6.7.8","countryCode":"ES"}"#;
        let st = super::parse_ipapi(json, 33).unwrap();
        assert_eq!(st, ProxyStatus::Ok { ip: "5.6.7.8".into(), country: "ES".into(), latency_ms: 33 });
    }

    #[test]
    fn fail_status_maps_to_dead() {
        let json = r#"{"status":"fail"}"#;
        assert_eq!(super::parse_ipapi(json, 0).unwrap(), ProxyStatus::Dead);
    }

    #[test]
    fn parse_ipapi_geo_extracts_tz_and_locale() {
        let json = r#"{"status":"success","timezone":"Europe/Madrid","countryCode":"ES","query":"1.2.3.4"}"#;
        let (tz, locale) = super::parse_ipapi_geo(json);
        assert_eq!(tz, Some("Europe/Madrid".into()));
        assert_eq!(locale, Some("es-ES".into())); // match_country("ES").locale
    }

    #[test]
    fn parse_ipapi_geo_handles_fail() {
        assert_eq!(super::parse_ipapi_geo(r#"{"status":"fail"}"#), (None, None));
    }

    #[tokio::test]
    #[ignore = "requires a working proxy at $RUSTCLOAK_TEST_PROXY"]
    async fn live_check_returns_ok() {
        let proxy = std::env::var("RUSTCLOAK_TEST_PROXY").unwrap();
        let st = super::check(&proxy).await.unwrap();
        assert!(matches!(st, ProxyStatus::Ok { .. }));
    }
}

use std::time::Instant;

/// Perform a live proxy check by requesting ip-api.com through the proxy.
pub async fn check(proxy: &str) -> Result<ProxyStatus> {
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(proxy)?)
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let start = Instant::now();
    let resp = match client.get("http://ip-api.com/json").send().await {
        Ok(r) => r,
        Err(_) => return Ok(ProxyStatus::Dead),
    };
    let latency = start.elapsed().as_millis() as u32;
    let body = resp.text().await?;
    parse_ipapi(&body, latency)
}
