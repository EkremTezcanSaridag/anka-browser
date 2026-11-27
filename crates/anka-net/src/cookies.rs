// Anka Browser - Çerez Kutusu (Cookie Jar) ve Oturum Yönetimi (RFC 6265 Temelleri - v0.3.1)
// Sıfır üçüncü parti crate ile sunucudan gelen `Set-Cookie` başlıklarını ayrıştırır,
// domain/path/secure güvenlik kurallarını uygular ve sonraki isteklerde `Cookie:` başlığı üretir.

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
                    "max-age" if val_t.parse::<i64>().is_ok_and(|sec| sec <= 0) => {
                        expired = true;
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
            });
        }
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

        let mut matched: Vec<String> = Vec::new();
        for c in &self.cookies {
            let domain_ok = host == c.domain || host.ends_with(&format!(".{}", c.domain));
            let path_ok = url.path.starts_with(&c.path);
            let secure_ok = !c.secure || is_https;
            if domain_ok && path_ok && secure_ok {
                matched.push(format!("{}={}", c.name, c.value));
            }
        }

        if matched.is_empty() {
            None
        } else {
            Some(matched.join("; "))
        }
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
}
