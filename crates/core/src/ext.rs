use anyhow::{anyhow, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtSource {
    WebStore(String), // extension id
    CrxUrl(String),
    ZipUrl(String),
}

/// Classify a user-provided extension source string.
pub fn detect_source(input: &str) -> Result<ExtSource> {
    let s = input.trim();
    // Path without query/fragment, e.g. ".../<id>?hl=ru" → ".../<id>".
    let path = s.split(['?', '#']).next().unwrap_or(s);
    // Chrome Web Store URL: .../detail/<slug>/<32-char-id>
    if s.contains("chromewebstore.google.com") || s.contains("chrome.google.com/webstore") {
        let id = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        if is_ext_id(id) {
            return Ok(ExtSource::WebStore(id.to_string()));
        }
        return Err(anyhow!("could not extract extension id from web store url"));
    }
    if is_ext_id(path) {
        return Ok(ExtSource::WebStore(path.to_string()));
    }
    // Keep the full URL (with query) for download — only the path decides the type.
    if path.ends_with(".crx") {
        return Ok(ExtSource::CrxUrl(s.to_string()));
    }
    if path.ends_with(".zip") {
        return Ok(ExtSource::ZipUrl(s.to_string()));
    }
    Err(anyhow!("unrecognized extension source: {s}"))
}

fn is_ext_id(s: &str) -> bool {
    s.len() == 32 && s.chars().all(|c| ('a'..='p').contains(&c))
}

/// Build the CRX download URL for a Web Store extension id.
pub fn webstore_crx_url(id: &str) -> String {
    format!(
        "https://clients2.google.com/service/update2/crx?response=redirect&acceptformat=crx2,crx3&prodversion=120.0&x=id%3D{id}%26installsource%3Dondemand%26uc"
    )
}

/// Strip the CRX3 header, returning the embedded ZIP bytes.
/// CRX3 layout: magic "Cr24" (4) + version u32le (4) + header_len u32le (4) + header + zip.
pub fn strip_crx_header(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.len() < 12 || &bytes[0..4] != b"Cr24" {
        // Not a CRX — assume it is already a raw zip.
        return Ok(bytes.to_vec());
    }
    let header_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let zip_start = 12 + header_len;
    if bytes.len() < zip_start {
        return Err(anyhow!("crx header length exceeds file size"));
    }
    Ok(bytes[zip_start..].to_vec())
}

// ---- CRX3 public-key extraction (for stable, path-independent extension IDs) ----

fn read_varint(buf: &[u8], mut i: usize) -> Option<(u64, usize)> {
    let start = i;
    let (mut result, mut shift) = (0u64, 0u32);
    loop {
        let b = *buf.get(i)?;
        i += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return None;
        }
    }
    Some((result, i - start))
}

/// Return the byte slices of every length-delimited (wire type 2) field with the
/// given field number in a protobuf message.
fn pb_fields(buf: &[u8], field: u64) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < buf.len() {
        let (tag, n) = match read_varint(buf, i) {
            Some(v) => v,
            None => break,
        };
        i += n;
        let (field_no, wire) = (tag >> 3, tag & 7);
        match wire {
            2 => {
                let (len, n2) = match read_varint(buf, i) {
                    Some(v) => v,
                    None => break,
                };
                i += n2;
                let len = len as usize;
                if let Some(seg) = buf.get(i..i + len) {
                    if field_no == field {
                        out.push(seg);
                    }
                } else {
                    break;
                }
                i += len;
            }
            0 => {
                match read_varint(buf, i) {
                    Some((_, n2)) => i += n2,
                    None => break,
                }
            }
            1 => i += 8,
            5 => i += 4,
            _ => break,
        }
    }
    out
}

fn pb_first(buf: &[u8], field: u64) -> Option<&[u8]> {
    pb_fields(buf, field).into_iter().next()
}

/// The Chrome extension ID derived from a DER SPKI public key:
/// first 16 bytes of SHA-256, each nibble mapped 0..f -> a..p.
pub fn id_from_pubkey(public_key_der: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(public_key_der);
    let mut id = String::with_capacity(32);
    for &b in &digest[..16] {
        id.push((b'a' + (b >> 4)) as char);
        id.push((b'a' + (b & 0x0f)) as char);
    }
    id
}

/// Extract the developer RSA public key (DER SPKI) from a CRX3 file — the one
/// whose hash equals the declared crx_id. Returns None if `bytes` isn't a CRX3
/// or no matching key is found. Used to pin a stable extension ID via the
/// manifest "key" field, so the ID survives path changes (e.g. cloning).
pub fn crx_public_key(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 12 || &bytes[0..4] != b"Cr24" {
        return None;
    }
    let header_len = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let header = bytes.get(12..12 + header_len)?;
    // signed_header_data (field 10000) -> SignedData { crx_id = field 1, 16 bytes }
    let crx_id = pb_first(header, 10000).and_then(|sd| pb_first(sd, 1));
    let proofs = pb_fields(header, 2); // sha256_with_rsa AsymmetricKeyProof
    let mut first_key: Option<Vec<u8>> = None;
    for proof in proofs {
        if let Some(pk) = pb_first(proof, 1) {
            if first_key.is_none() {
                first_key = Some(pk.to_vec());
            }
            if let Some(want) = crx_id {
                use sha2::{Digest, Sha256};
                if &Sha256::digest(pk)[..16] == want {
                    return Some(pk.to_vec());
                }
            }
        }
    }
    // No crx_id match (or no crx_id present): fall back to the first key.
    first_key
}

/// Inject a `"key"` into the extension's manifest.json (if absent) so Chromium
/// derives a stable, path-independent ID. Best-effort: returns Ok even if the
/// manifest can't be parsed (some manifests have comments) — the extension still
/// loads, just with a path-derived ID.
pub fn inject_manifest_key(ext_dir: &Path, public_key_der: &[u8]) -> Result<()> {
    use base64::Engine;
    let manifest_path = ext_dir.join("manifest.json");
    let text = match std::fs::read_to_string(&manifest_path) {
        Ok(t) => t,
        Err(_) => return Ok(()),
    };
    let trimmed = text.trim_start_matches('\u{feff}');
    let mut v: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };
    if v.get("key").is_some() {
        return Ok(());
    }
    if let Some(obj) = v.as_object_mut() {
        let b64 = base64::engine::general_purpose::STANDARD.encode(public_key_der);
        obj.insert("key".to_string(), serde_json::Value::String(b64));
        std::fs::write(&manifest_path, serde_json::to_string_pretty(&v)?)?;
    }
    Ok(())
}

use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Pick a stable subfolder name for a source. Web Store extensions use their id;
/// other sources fall back to a positional name so multiple coexist.
pub fn folder_name_for(source: &str, index: usize) -> String {
    match detect_source(source) {
        Ok(ExtSource::WebStore(id)) => id,
        _ => format!("ext{index}"),
    }
}

/// Unzip raw zip bytes into `dest_dir`. Returns the directory written to.
pub fn unzip_into(zip_bytes: &[u8], dest_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))?;
    archive.extract(dest_dir)?;
    Ok(dest_dir.to_path_buf())
}

/// Download and install an extension into `<extensions_dir>/<name>`.
/// `name` is used as the subfolder (e.g. the extension id).
pub async fn install(source: &str, extensions_dir: &Path, name: &str) -> Result<PathBuf> {
    let src = detect_source(source)?;
    let url = match &src {
        ExtSource::WebStore(id) => webstore_crx_url(id),
        ExtSource::CrxUrl(u) | ExtSource::ZipUrl(u) => u.clone(),
    };
    let bytes = reqwest::get(&url).await?.bytes().await?;
    let pubkey = crx_public_key(&bytes); // before stripping the header
    let zip_bytes = strip_crx_header(&bytes)?; // CRX/zip both handled (non-CRX passes through)
    let dest = extensions_dir.join(name);
    unzip_into(&zip_bytes, &dest)?;
    // Pin a stable extension ID so it survives path changes (cloning), keeping
    // the extension's authorized state working in clones.
    if let Some(key) = pubkey {
        let _ = inject_manifest_key(&dest, &key);
    }
    Ok(dest)
}

/// Install a list of extension sources into `extensions_dir`, one subfolder each.
/// Returns the list of installed dirs. A failed source is collected as an error
/// string rather than aborting the whole batch.
pub async fn install_many(sources: &[String], extensions_dir: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let mut installed = Vec::new();
    let mut errors = Vec::new();
    for (i, source) in sources.iter().enumerate() {
        let name = folder_name_for(source, i);
        match install(source, extensions_dir, &name).await {
            Ok(p) => installed.push(p),
            Err(e) => errors.push(format!("{source}: {e}")),
        }
    }
    (installed, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc_varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }
    fn enc_field(field: u64, data: &[u8]) -> Vec<u8> {
        let mut out = enc_varint((field << 3) | 2);
        out.extend(enc_varint(data.len() as u64));
        out.extend_from_slice(data);
        out
    }

    #[test]
    fn id_from_pubkey_maps_nibbles_to_a_p() {
        let id = id_from_pubkey(b"anything");
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| ('a'..='p').contains(&c)));
    }

    #[test]
    fn crx_public_key_picks_key_matching_crx_id() {
        use sha2::{Digest, Sha256};
        let key_a = vec![0xAAu8; 40]; // decoy (e.g. Google's key)
        let key_b = vec![0xBBu8; 40]; // developer key
        let crx_id = Sha256::digest(&key_b)[..16].to_vec();

        // header = proof(A) + proof(B) + signed_header_data{crx_id}
        let mut header = Vec::new();
        header.extend(enc_field(2, &enc_field(1, &key_a)));
        header.extend(enc_field(2, &enc_field(1, &key_b)));
        header.extend(enc_field(10000, &enc_field(1, &crx_id)));

        let mut crx = Vec::new();
        crx.extend_from_slice(b"Cr24");
        crx.extend_from_slice(&3u32.to_le_bytes());
        crx.extend_from_slice(&(header.len() as u32).to_le_bytes());
        crx.extend_from_slice(&header);

        assert_eq!(crx_public_key(&crx), Some(key_b));
    }

    #[test]
    fn inject_manifest_key_adds_key_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("manifest.json"), r#"{"name":"x","version":"1"}"#).unwrap();
        inject_manifest_key(dir.path(), b"pubkeybytes").unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join("manifest.json")).unwrap()).unwrap();
        assert!(v.get("key").and_then(|k| k.as_str()).is_some());
        // idempotent: a second call keeps the existing key
        let before = std::fs::read_to_string(dir.path().join("manifest.json")).unwrap();
        inject_manifest_key(dir.path(), b"different").unwrap();
        assert_eq!(before, std::fs::read_to_string(dir.path().join("manifest.json")).unwrap());
    }

    #[tokio::test]
    #[ignore = "network: downloads the VPN extension CRX and checks the derived id"]
    async fn live_crx_key_yields_web_store_id() {
        let id = "omghfjlpggmjjaagoclmmobgdodcjboh";
        let bytes = reqwest::get(super::webstore_crx_url(id)).await.unwrap().bytes().await.unwrap();
        let key = super::crx_public_key(&bytes).expect("crx key");
        assert_eq!(super::id_from_pubkey(&key), id);
    }

    #[test]
    fn detects_web_store_url_with_query() {
        // Localized slug + ?hl=ru query must not corrupt the id.
        let url = "https://chromewebstore.google.com/detail/vpn-proxy/omghfjlpggmjjaagoclmmobgdodcjboh?hl=ru";
        assert_eq!(
            detect_source(url).unwrap(),
            ExtSource::WebStore("omghfjlpggmjjaagoclmmobgdodcjboh".into())
        );
    }

    #[test]
    fn detects_web_store_url() {
        let url = "https://chromewebstore.google.com/detail/ublock/cjpalhdlnbpafiamejdnhcphjbkeiagm";
        assert_eq!(detect_source(url).unwrap(), ExtSource::WebStore("cjpalhdlnbpafiamejdnhcphjbkeiagm".into()));
    }

    #[test]
    fn detects_bare_id_and_crx_and_zip() {
        assert_eq!(detect_source("cjpalhdlnbpafiamejdnhcphjbkeiagm").unwrap(),
                   ExtSource::WebStore("cjpalhdlnbpafiamejdnhcphjbkeiagm".into()));
        assert_eq!(detect_source("https://x.com/e.crx").unwrap(), ExtSource::CrxUrl("https://x.com/e.crx".into()));
        assert_eq!(detect_source("https://x.com/e.zip").unwrap(), ExtSource::ZipUrl("https://x.com/e.zip".into()));
    }

    #[test]
    fn strips_crx3_header_to_reach_zip() {
        // Cr24 + version=3 + header_len=2 + 2 header bytes + "PKzip"
        let mut crx = Vec::new();
        crx.extend_from_slice(b"Cr24");
        crx.extend_from_slice(&3u32.to_le_bytes());
        crx.extend_from_slice(&2u32.to_le_bytes());
        crx.extend_from_slice(&[0xAA, 0xBB]);
        crx.extend_from_slice(b"PK\x03\x04zipdata");
        let zip = strip_crx_header(&crx).unwrap();
        assert_eq!(&zip[0..4], b"PK\x03\x04");
    }

    #[test]
    fn non_crx_passes_through() {
        let raw = b"PK\x03\x04hello";
        assert_eq!(strip_crx_header(raw).unwrap(), raw.to_vec());
    }

    #[test]
    fn folder_name_uses_id_or_positional() {
        assert_eq!(super::folder_name_for("cjpalhdlnbpafiamejdnhcphjbkeiagm", 0),
                   "cjpalhdlnbpafiamejdnhcphjbkeiagm");
        assert_eq!(super::folder_name_for("https://x.com/e.crx", 2), "ext2");
    }

    #[test]
    fn unzip_writes_files() {
        use std::io::Write;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            w.start_file::<_, ()>("manifest.json", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"{}").unwrap();
            w.finish().unwrap();
        }
        let dir = tempfile::tempdir().unwrap();
        let out = super::unzip_into(&buf, dir.path()).unwrap();
        assert!(out.join("manifest.json").exists());
    }

    #[tokio::test]
    #[ignore = "network: downloads uBlock Origin Lite from the Chrome Web Store"]
    async fn live_install_ublock() {
        let id = "ddkjiahejlhfcafbddmgiahcphecmpfh";
        let dir = tempfile::tempdir().unwrap();
        let out = super::install(id, dir.path(), id).await.unwrap();
        assert!(out.join("manifest.json").exists());
    }
}
