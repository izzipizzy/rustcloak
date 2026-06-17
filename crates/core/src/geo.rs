#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoProfile {
    pub timezone: String,
    pub locale: String,
}

/// Map an ISO country code to a representative timezone + locale.
/// Covers the common cases; unknown codes fall back to UTC/en-US.
pub fn match_country(country_code: &str) -> GeoProfile {
    let (tz, locale) = match country_code.to_uppercase().as_str() {
        "ES" => ("Europe/Madrid", "es-ES"),
        "US" => ("America/New_York", "en-US"),
        "GB" => ("Europe/London", "en-GB"),
        "DE" => ("Europe/Berlin", "de-DE"),
        "FR" => ("Europe/Paris", "fr-FR"),
        "IT" => ("Europe/Rome", "it-IT"),
        "BR" => ("America/Sao_Paulo", "pt-BR"),
        "RU" => ("Europe/Moscow", "ru-RU"),
        _ => ("UTC", "en-US"),
    };
    GeoProfile { timezone: tz.to_string(), locale: locale.to_string() }
}

/// Map an ISO country code to a BCP 47 locale (covers ~90% of traffic).
/// Falls back to en-US for unknown codes. Mirrors the CloakBrowser wrapper map.
pub fn locale_for_country(country_code: &str) -> String {
    let l = match country_code.to_uppercase().as_str() {
        "US" => "en-US", "GB" => "en-GB", "AU" => "en-AU", "CA" => "en-CA", "NZ" => "en-NZ",
        "IE" => "en-IE", "ZA" => "en-ZA", "SG" => "en-SG", "PH" => "en-PH",
        "DE" => "de-DE", "AT" => "de-AT", "CH" => "de-CH",
        "FR" => "fr-FR", "BE" => "fr-BE",
        "ES" => "es-ES", "MX" => "es-MX", "AR" => "es-AR", "CO" => "es-CO", "CL" => "es-CL",
        "BR" => "pt-BR", "PT" => "pt-PT",
        "IT" => "it-IT", "NL" => "nl-NL",
        "JP" => "ja-JP", "KR" => "ko-KR", "CN" => "zh-CN", "TW" => "zh-TW", "HK" => "zh-HK",
        "RU" => "ru-RU", "UA" => "uk-UA", "PL" => "pl-PL", "CZ" => "cs-CZ", "RO" => "ro-RO",
        "IL" => "he-IL", "TR" => "tr-TR", "SA" => "ar-SA", "AE" => "ar-AE", "EG" => "ar-EG",
        "IN" => "hi-IN", "ID" => "id-ID",
        "TH" => "th-TH", "VN" => "vi-VN", "MY" => "ms-MY",
        "SE" => "sv-SE", "NO" => "nb-NO", "DK" => "da-DK", "FI" => "fi-FI",
        "GR" => "el-GR", "HU" => "hu-HU", "BG" => "bg-BG",
        _ => "en-US",
    };
    l.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_country() {
        assert_eq!(match_country("ES"), GeoProfile { timezone: "Europe/Madrid".into(), locale: "es-ES".into() });
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(match_country("es").timezone, "Europe/Madrid");
    }

    #[test]
    fn unknown_falls_back_to_utc() {
        assert_eq!(match_country("ZZ"), GeoProfile { timezone: "UTC".into(), locale: "en-US".into() });
    }

    #[test]
    fn locale_for_country_covers_more_and_defaults() {
        assert_eq!(locale_for_country("FI"), "fi-FI");
        assert_eq!(locale_for_country("fi"), "fi-FI");
        assert_eq!(locale_for_country("JP"), "ja-JP");
        assert_eq!(locale_for_country("ZZ"), "en-US");
    }
}
