// Anka Browser - Yerleşik Reklam ve İzleyici Engelleyici ("Anka Kalkan" / Shield - v0.2.11)
// Sıfır üçüncü parti crate bağımlılığı ile:
// 1. Bilinen reklam/telemetri alan adlarını TCP/TLS bağlantısı açmadan ağ katmanında keser.
// 2. Linklerdeki `utm_*`, `fbclid`, `gclid` gibi izleme parametrelerini temizler.
// 3. 1x1 gizli takip piksellerini ve kesin reklam kutularını DOM üzerinden temizler.

use anka_url::Url;

/// Tek bir sayfa yüklemesi için Kalkan koruma istatistikleri:
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShieldStats {
    pub blocked_requests: usize,
    pub stripped_params: usize,
    pub blocked_dom_nodes: usize,
    pub blocked_hosts: Vec<String>,
}

impl ShieldStats {
    pub fn total_blocked(&self) -> usize {
        self.blocked_requests + self.stripped_params + self.blocked_dom_nodes
    }

    pub fn record_request(&mut self, host: &str) {
        self.blocked_requests += 1;
        let clean = host.trim().to_lowercase();
        if !clean.is_empty() && !self.blocked_hosts.contains(&clean) && self.blocked_hosts.len() < 16 {
            self.blocked_hosts.push(clean);
        }
    }
}

const TRACKER_DOMAINS: &[&str] = &[
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "google-analytics.com",
    "googletagmanager.com",
    "googletagservices.com",
    "adservice.google.com",
    "connect.facebook.net",
    "pixel.facebook.com",
    "scorecardresearch.com",
    "quantserve.com",
    "taboola.com",
    "outbrain.com",
    "criteo.com",
    "criteo.net",
    "pubmatic.com",
    "rubiconproject.com",
    "openx.net",
    "adsrvr.org",
    "adnxs.com",
    "amazon-adsystem.com",
    "hotjar.com",
    "mixpanel.com",
    "segment.io",
    "segment.com",
    "amplitude.com",
    "clarity.ms",
    "mc.yandex.ru",
    "yandexadexchange.net",
    "adform.net",
    "mgid.com",
    "revcontent.com",
];

const BLOCKED_PATH_MARKERS: &[&str] = &[
    "/pagead/",
    "/adserver/",
    "/gtag/js",
    "analytics.js",
    "fbevents.js",
    "adsense.js",
    "collect.gif",
];

const TRACKING_QUERY_KEYS: &[&str] = &[
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_term",
    "utm_content",
    "utm_id",
    "fbclid",
    "gclid",
    "dclid",
    "msclkid",
    "mc_cid",
    "mc_eid",
    "_ga",
    "_gl",
    "igshid",
    "yclid",
    "ref_src",
    "si",
];

const COSMETIC_AD_TOKENS: &[&str] = &[
    "ad",
    "ads",
    "ad-box",
    "ad-banner",
    "ad-slot",
    "ad-container",
    "ad-wrapper",
    "advertisement",
    "adsbygoogle",
    "sponsored",
    "sponsored-post",
    "sponsored-content",
    "taboola-widget",
    "outbrain-widget",
];

/// Verilen `Url`'nin reklam, analitik veya izleyici ağına gidip gitmediğini kontrol eder.
pub fn is_blocked_url(url: &Url) -> bool {
    if url.host == "anka.local" || url.scheme == "file" {
        return false;
    }

    let host = url.host.trim().to_lowercase();
    if TRACKER_DOMAINS
        .iter()
        .any(|&domain| host == domain || host.ends_with(&format!(".{domain}")))
    {
        return true;
    }

    let path_lower = url.path.to_lowercase();
    BLOCKED_PATH_MARKERS
        .iter()
        .any(|marker| path_lower.contains(marker))
}

/// URL sorgu parametrelerindeki `utm_*`, `fbclid`, `gclid` vb. izleme anahtarlarını siler.
/// Geriye temizlenmiş URL'yi ve silinen parametre sayısını döndürür.
pub fn strip_tracking_params(raw_url: &str) -> (String, usize) {
    let (before_hash, fragment) = match raw_url.split_once('#') {
        Some((base, frag)) => (base, Some(frag)),
        None => (raw_url, None),
    };

    let Some((base_path, query)) = before_hash.split_once('?') else {
        return (raw_url.to_string(), 0);
    };

    let mut kept = Vec::new();
    let mut removed = 0usize;

    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let key = pair.split('=').next().unwrap_or("").trim().to_lowercase();
        if TRACKING_QUERY_KEYS.contains(&key.as_str()) {
            removed += 1;
        } else {
            kept.push(pair);
        }
    }

    if removed == 0 {
        return (raw_url.to_string(), 0);
    }

    let mut cleaned = base_path.to_string();
    if !kept.is_empty() {
        cleaned.push('?');
        cleaned.push_str(&kept.join("&"));
    }
    if let Some(frag) = fragment {
        cleaned.push('#');
        cleaned.push_str(frag);
    }
    (cleaned, removed)
}

/// HTML `class="..."` veya `id="..."` değerinde kesin reklam kutusu belirteci olup olmadığını kontrol eder.
/// Tam token karşılaştırması yaptığı için `"header"` veya `"address"` gibi kelimeleri yanlışlıkla engellemez!
pub fn is_cosmetic_ad_element(class_attr: Option<&str>, id_attr: Option<&str>) -> bool {
    if let Some(classes) = class_attr {
        for token in classes.split_whitespace() {
            let clean = token.trim().to_lowercase();
            if COSMETIC_AD_TOKENS.contains(&clean.as_str()) {
                return true;
            }
        }
    }
    if let Some(id) = id_attr {
        let clean = id.trim().to_lowercase();
        if COSMETIC_AD_TOKENS.contains(&clean.as_str()) {
            return true;
        }
    }
    false
}

/// 1x1 veya 0x0 boyutlu görünmez izleme piksel görsellerini (`<img width="1" height="1">`) yakalar.
pub fn is_tracking_pixel(width_attr: Option<&str>, height_attr: Option<&str>) -> bool {
    let parse_dim = |raw: Option<&str>| -> Option<u32> {
        raw?.trim().trim_end_matches("px").trim().parse::<u32>().ok()
    };
    matches!((parse_dim(width_attr), parse_dim(height_attr)), (Some(w), Some(h)) if w <= 1 && h <= 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reklam_ve_izleyici_domain_engelleme() {
        let tracker = Url::parse("https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js")
            .expect("URL gecersiz");
        let analytics =
            Url::parse("https://www.google-analytics.com/analytics.js").expect("URL gecersiz");
        let normal = Url::parse("https://tr.wikipedia.org/wiki/Anka").expect("URL gecersiz");
        let local = Url::parse("http://anka.local/kalkan").expect("URL gecersiz");

        assert!(is_blocked_url(&tracker));
        assert!(is_blocked_url(&analytics));
        assert!(!is_blocked_url(&normal));
        assert!(!is_blocked_url(&local));
    }

    #[test]
    fn test_utm_ve_fbclid_parametre_temizleme() {
        let kirli = "https://example.com/haber?id=42&utm_source=x&fbclid=abc123&lang=tr#bolum";
        let (temiz, silinen) = strip_tracking_params(kirli);
        assert_eq!(silinen, 2);
        assert_eq!(temiz, "https://example.com/haber?id=42&lang=tr#bolum");

        let yalniz_izleyici = "https://example.com/?utm_medium=social";
        let (temiz2, silinen2) = strip_tracking_params(yalniz_izleyici);
        assert_eq!(silinen2, 1);
        assert_eq!(temiz2, "https://example.com/");
    }

    #[test]
    fn test_kozmetik_ve_izleme_pikseli_guvenli_tespit() {
        assert!(is_cosmetic_ad_element(Some("card advertisement"), None));
        assert!(is_cosmetic_ad_element(None, Some("ad-banner")));
        assert!(!is_cosmetic_ad_element(Some("header address download"), Some("main-header")));

        assert!(is_tracking_pixel(Some("1"), Some("1")));
        assert!(!is_tracking_pixel(Some("200"), Some("1")));
        assert!(!is_tracking_pixel(None, None));
    }
}
