// Anka Browser - Çerez Kutusu (Cookie Jar) ve Oturum Yönetimi (RFC 6265 Temelleri - v0.3.2)
// Sıfır üçüncü parti crate ile sunucudan gelen `Set-Cookie` başlıklarını ayrıştırır,
// domain/path/secure/Expires/Max-Age kurallarını uygular, TSV formatında diske kaydeder
// ve sonraki isteklerde `Cookie:` başlığı üretir.

use std::time::{SystemTime, UNIX_EPOCH};

use anka_url::Url;

/// Tek bir HTTP çerez kaydı:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    /// `Max-Age` veya `Expires` varsa Unix zaman damgası (saniye); `None` ise oturum çerezidir.
    pub expires_epoch: Option<u64>,
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn month_number(mon: &str) -> Option<i64> {
    match mon.to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

/// Standart `Expires` HTTP tarihini (`Wed, 21 Oct 2026 07:28:00 GMT` veya tireli biçim)
/// Unix zaman damgasına (saniye) çevirir.
pub fn parse_http_date_epoch(raw: &str) -> Option<u64> {
    let after_comma = raw
        .split_once(',')
        .map(|(_, rest)| rest.trim())
        .unwrap_or_else(|| raw.trim());
    // "21-Oct-2026 07:28:00 GMT" gibi tireli tarih ayraçlarını boşluğa çevir:
    let mut normalized = String::with_capacity(after_comma.len());
    for ch in after_comma.chars() {
        normalized.push(if ch == '-' { ' ' } else { ch });
    }
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }

    let day = parts[0].parse::<i64>().ok()?;
    let month = month_number(parts[1])?;
    let year = parts[2].parse::<i64>().ok()?;
    if !(1..=31).contains(&day) || year < 0 {
        return None;
    }

    let mut time_parts = parts[3].split(':');
    let hh = time_parts.next()?.parse::<u64>().ok()?;
    let mm = time_parts.next()?.parse::<u64>().ok()?;
    let ss = time_parts.next()?.parse::<u64>().ok()?;
    if hh >= 24 || mm >= 60 || ss >= 60 {
        return None;
    }

    if year < 1970 {
        return Some(0);
    }
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    if days < 0 {
        return Some(0);
    }

    Some((days as u64) * 86_400 + hh * 3_600 + mm * 60 + ss)
}

/// Tarayıcının tüm sekmeler arasında paylaştığı çerez deposu (`CookieJar`):
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CookieJar {
    cookies: Vec<Cookie>,
}

impl CookieJar {
    pub fn new() -> Self {
        Self {
            cookies: Vec::new(),
        }
    }

    pub fn count(&self) -> usize {
        self.cookies.len()
    }

    pub fn all(&self) -> &[Cookie] {
        &self.cookies
    }

    pub fn clear(&mut self) {
        self.cookies.clear();
    }

    /// Bir URL ve tek bir ham `Set-Cookie` başlık değeri (`"sid=abc; Path=/; HttpOnly"`) alarak
    /// çerezi depolar veya `Max-Age=0` / boş silme geldiğinde kaldırır.
    pub fn store_header_line(&mut self, url: &Url, raw_set_cookie: &str) {
        let mut parts = raw_set_cookie.split(';');
        let Some(first_pair) = parts.next() else {
            return;
        };
        let Some((raw_name, raw_val)) = first_pair.split_once('=') else {
            return;
        };
        let name = raw_name.trim().to_string();
        let value = raw_val.trim().to_string();
        if name.is_empty() {
            return;
        }

        let req_host = url.host.trim().to_lowercase();
        if req_host.is_empty() {
            return;
        }

        let mut domain = req_host.clone();
        let mut path = "/".to_string();
        let mut secure = false;
        let mut http_only = false;
        let mut expired = false;
        let mut max_age_seen = false;
        let mut expires_epoch: Option<u64> = None;
        let now = now_epoch_secs();

        for attr in parts {
            let trimmed = attr.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                let key_l = k.trim().to_lowercase();
                let val_t = v.trim();
                match key_l.as_str() {
                    "domain" => {
                        let clean_dom = val_t.trim_start_matches('.').to_lowercase();
                        // Güvenlik: Başka bir sitenin alan adı adına çerez bırakılmasını engelle!
                        if !clean_dom.is_empty()
                            && (req_host == clean_dom || req_host.ends_with(&format!(".{clean_dom}")))
                        {
                            domain = clean_dom;
                        }
                    }
                    "path" if val_t.starts_with('/') => {
                        path = val_t.to_string();
                    }
                    "max-age" => {
                        if let Ok(sec) = val_t.parse::<i64>() {
                            max_age_seen = true;
                            if sec <= 0 {
                                expired = true;
                            } else {
                                expires_epoch = Some(now.saturating_add(sec as u64));
                            }
                        }
                    }
                    "expires" if !max_age_seen => {
                        if let Some(epoch) = parse_http_date_epoch(val_t) {
                            if epoch <= now {
                                expired = true;
                            } else {
                                expires_epoch = Some(epoch);
                            }
                        }
                    }
                    _ => {}
                }
            } else {
                match trimmed.to_lowercase().as_str() {
                    "secure" => secure = true,
                    "httponly" => http_only = true,
                    _ => {}
                }
            }
        }

        self.cookies
            .retain(|c| !(c.domain == domain && c.path == path && c.name == name));

        if !expired {
            self.cookies.push(Cookie {
                name,
                value,
                domain,
                path,
                secure,
                http_only,
                expires_epoch,
            });
        }
    }

    /// Süresi dolmuş çerezleri kavanozdan temizler.
    pub fn prune_expired(&mut self) {
        let now = now_epoch_secs();
        self.cookies
            .retain(|c| c.expires_epoch.is_none_or(|exp| exp > now));
    }

    /// HTTP yanıtındaki tüm `Set-Cookie` başlıklarını işler.
    pub fn store_from_headers(&mut self, url: &Url, headers: &[(String, String)]) {
        for (key, value) in headers {
            if key.eq_ignore_ascii_case("set-cookie") {
                self.store_header_line(url, value);
            }
        }
    }

    /// Hedef `Url` için geçerli çerezleri toplayıp `"isim1=deger1; isim2=deger2"` döndürür.
    pub fn cookie_header_for(&self, url: &Url) -> Option<String> {
        if url.scheme == "file" {
            return None;
        }
        let host = url.host.trim().to_lowercase();
        let is_https = url.scheme == "https" || host == "anka.local";
        let now = now_epoch_secs();

        let mut matched: Vec<String> = Vec::new();
        for c in &self.cookies {
            let not_expired = c.expires_epoch.is_none_or(|exp| exp > now);
            let domain_ok = host == c.domain || host.ends_with(&format!(".{}", c.domain));
            let path_ok = url.path.starts_with(&c.path);
            let secure_ok = !c.secure || is_https;
            if not_expired && domain_ok && path_ok && secure_ok {
                matched.push(format!("{}={}", c.name, c.value));
            }
        }

        if matched.is_empty() {
            None
        } else {
            Some(matched.join("; "))
        }
    }

    /// Kalıcı (süresi henüz dolmamış `expires_epoch` taşıyan) çerezleri TSV metnine çevirir.
    pub fn serialize_tsv(&self) -> String {
        let now = now_epoch_secs();
        let sanitize = |s: &str| s.replace(['\t', '\r', '\n'], " ");
        let mut out = String::new();
        for c in &self.cookies {
            if let Some(exp) = c.expires_epoch
                && exp > now
            {
                out.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    sanitize(&c.domain),
                    sanitize(&c.path),
                    sanitize(&c.name),
                    sanitize(&c.value),
                    u8::from(c.secure),
                    u8::from(c.http_only),
                    exp
                ));
            }
        }
        out
    }

    /// Diskten okunan TSV verisinden yalnızca süresi dolmamış kalıcı çerezleri geri yükler.
    pub fn deserialize_tsv(raw: &str) -> Self {
        let now = now_epoch_secs();
        let mut cookies = Vec::new();
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let p: Vec<&str> = trimmed.split('\t').collect();
            if p.len() != 7 {
                continue;
            }
            let Ok(exp) = p[6].parse::<u64>() else {
                continue;
            };
            if exp <= now || p[0].is_empty() || p[2].is_empty() {
                continue;
            }
            cookies.push(Cookie {
                domain: p[0].to_string(),
                path: p[1].to_string(),
                name: p[2].to_string(),
                value: p[3].to_string(),
                secure: p[4] == "1",
                http_only: p[5] == "1",
                expires_epoch: Some(exp),
            });
        }
        Self { cookies }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cerez_saklama_ve_geri_gonderme() {
        let mut jar = CookieJar::new();
        let login_url = Url::parse("https://auth.example.com/login").unwrap();
        let headers = vec![
            (
                "Set-Cookie".to_string(),
                "sid=anka_42; Domain=.example.com; Path=/; HttpOnly; Secure".to_string(),
            ),
            (
                "Set-Cookie".to_string(),
                "evil=1; Domain=baskasite.org; Path=/".to_string(),
            ),
        ];

        jar.store_from_headers(&login_url, &headers);
        assert_eq!(jar.count(), 2);

        // Alt alan adına (`app.example.com`) `sid` gider ama `evil` asla eşleşmez!
        let app_url = Url::parse("https://app.example.com/panel").unwrap();
        assert_eq!(
            jar.cookie_header_for(&app_url).as_deref(),
            Some("sid=anka_42")
        );

        // Güvensiz `http://` üzerinden `Secure` çerez gönderilmez!
        let plain_url = Url::parse("http://app.example.com/panel").unwrap();
        assert_eq!(jar.cookie_header_for(&plain_url), None);

        // Farklı alan adı (`baskasite.org`) çerezi domain güvenlik kontrolünden geçemez!
        let evil_url = Url::parse("https://baskasite.org/").unwrap();
        assert_eq!(jar.cookie_header_for(&evil_url), None);
    }

    #[test]
    fn test_cerez_guncelleme_ve_max_age_sifir_silme() {
        let mut jar = CookieJar::new();
        let url = Url::parse("http://anka.local/oturum").unwrap();

        jar.store_header_line(&url, "anka_user=Ekrem; Path=/");
        assert_eq!(
            jar.cookie_header_for(&url).as_deref(),
            Some("anka_user=Ekrem")
        );

        jar.store_header_line(&url, "anka_user=Tezcan; Path=/");
        assert_eq!(jar.count(), 1);
        assert_eq!(
            jar.cookie_header_for(&url).as_deref(),
            Some("anka_user=Tezcan")
        );

        jar.store_header_line(&url, "anka_user=; Path=/; Max-Age=0");
        assert_eq!(jar.count(), 0);
        assert_eq!(jar.cookie_header_for(&url), None);
    }

    #[test]
    fn test_expires_tarih_ayristirma_ve_kalici_tsv_depolama() {
        let mut jar = CookieJar::new();
        let url = Url::parse("https://anka.local/oturum").unwrap();

        // Gelecek tarihli kalici cerez saklanmali, gecmis tarihli (1999) reddedilmeli:
        jar.store_header_line(
            &url,
            "tema=koyu; Path=/; Expires=Wed, 21 Oct 2099 07:28:00 GMT",
        );
        jar.store_header_line(
            &url,
            "eski=silinmeli; Path=/; Expires=Thu, 01 Jan 1999 00:00:00 GMT",
        );
        jar.store_header_line(&url, "gecici=oturum; Path=/");
        jar.store_header_line(&url, "token=7gun; Path=/; Max-Age=604800");

        assert_eq!(jar.count(), 3);
        let tsv = jar.serialize_tsv();
        // Yalnızca `Expires`/`Max-Age > 0` taşıyan kalıcı çerezler TSV'ye yazılır:
        assert!(tsv.contains("tema\tkoyu"));
        assert!(tsv.contains("token\t7gun"));
        assert!(!tsv.contains("gecici"));

        let restored = CookieJar::deserialize_tsv(&tsv);
        assert_eq!(restored.count(), 2);
        assert_eq!(
            restored.cookie_header_for(&url).as_deref(),
            Some("tema=koyu; token=7gun")
        );
    }
}
