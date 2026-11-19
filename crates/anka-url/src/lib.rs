// Anka Browser - URL Veri Yapısı (WHATWG URL Standardı Temeli)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url {
    // Protokol (örn: "http", "https")
    pub scheme: String,

    // Alan adı veya IP adresi (örn: "ornek.com")
    pub host: String,

    // Port kapı numarası (örn: 8080) - Yazılmamışsa None olur
    pub port: Option<u16>,

    // Dosya / sayfa yolu (örn: "/haberler" veya "/")
    pub path: String,

    // Soru işaretinden (?) sonraki sorgu (örn: "sayfa=2")
    pub query: Option<String>,

    // Diyez (#) işaretinden sonraki sayfa içi bölüm (örn: "yorumlar")
    pub fragment: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlError {
    // Adres tamamen boş bırakılmış (örn: "")
    EmptyUrl,
    // Adresin protokol kısmı eksik (örn: "www.example.com" veya "://example.com")
    MissingScheme,
    // Adresin host (site adı) kısmı eksik (örn: "http://" veya "http:///sayfa")
    MissingHost,
    // Port kısmı geçerli bir sayı değil (örn: "http://example.com:abc")
    InvalidPort,
}

impl Url {
    /// Verilen ham URL metnini (raw) parçalayarak `Url` paketi oluşturur.
    /// Örnek girdi: "https://ornek.com:8080/haberler?sayfa=2#yorumlar"
    pub fn parse(raw: &str) -> Result<Self, UrlError> {
        let raw = raw.trim();

        // 1. ADIM: Yazı tamamen boş mu?
        if raw.is_empty() {
            return Err(UrlError::EmptyUrl);
        }

        // 2. ADIM: Protokolü (scheme) "://" kısmından ayır
        // Örn: "https://ornek.com:8080/haberler?sayfa=2#yorumlar"
        //   -> scheme = "https"
        //   -> rest   = "ornek.com:8080/haberler?sayfa=2#yorumlar"
        let (scheme, rest) = match raw.split_once("://") {
            Some((s, r)) => (s, r),
            None => return Err(UrlError::MissingScheme),
        };

        if scheme.is_empty() {
            return Err(UrlError::MissingScheme);
        }

        // 3. ADIM: En sondaki "#fragment" (sayfa içi bölüm) kısmını ayır
        // Örn: "ornek.com:8080/haberler?sayfa=2#yorumlar"
        //   -> before_fragment = "ornek.com:8080/haberler?sayfa=2"
        //   -> fragment        = Some("yorumlar")
        let (before_fragment, fragment) = match rest.split_once('#') {
            Some((before, frag)) => (before, Some(frag.to_string())),
            None => (rest, None),
        };

        // 4. ADIM: "?query" (sorgu parametreleri) kısmını ayır
        // Örn: "ornek.com:8080/haberler?sayfa=2"
        //   -> before_query = "ornek.com:8080/haberler"
        //   -> query        = Some("sayfa=2")
        let (before_query, query) = match before_fragment.split_once('?') {
            Some((before, q)) => (before, Some(q.to_string())),
            None => (before_fragment, None),
        };

        // 5. ADIM: Site adı (host:port) ile sayfa yolunu (/path) ayır
        // Örn: "ornek.com:8080/haberler"
        //   -> host_and_port = "ornek.com:8080"
        //   -> path          = "/haberler" (Eğer hiç '/' yoksa varsayılan olarak "/" olur)
        let (host_and_port, path) = match before_query.split_once('/') {
            Some((hp, p)) => (hp, format!("/{p}")),
            None => (before_query, "/".to_string()),
        };

        if host_and_port.is_empty() {
            return Err(UrlError::MissingHost);
        }

        // 6. ADIM: Site adını (host) ve varsa kapı numarasını (:port) ayır
        // Örn: "ornek.com:8080" -> host = "ornek.com", port = Some(8080)
        let (host, port) = match host_and_port.split_once(':') {
            Some((h, p)) => {
                if h.is_empty() {
                    return Err(UrlError::MissingHost);
                }
                // Port yazısını 0-65535 arası sayıya (u16) çevirmeyi dene
                let port_num = match p.parse::<u16>() {
                    Ok(num) => num,
                    Err(_) => return Err(UrlError::InvalidPort),
                };
                (h.to_lowercase(), Some(port_num))
            }
            None => (host_and_port.to_lowercase(), None),
        };

        // 7. ADIM: Tüm parçalar hazır! Url paketini doldurup Ok(...) içinde döndür
        Ok(Url {
            scheme: scheme.to_lowercase(),
            host,
            port,
            path,
            query,
            fragment,
        })
    }

    /// Adreste port belirtilmişse onu döner; belirtilmemişse
    /// protokole göre varsayılan kapı numarasını (http -> 80, https -> 443) döner.
    pub fn effective_port(&self) -> Option<u16> {
        match self.port {
            Some(p) => Some(p),
            None => match self.scheme.as_str() {
                "http" => Some(80),
                "https" => Some(443),
                _ => None,
            },
        }
    }

    /// Mevcut sayfanın adresini (`self`) temel alarak, sayfa içindeki
    /// bağıl veya tam bir linki (`href`) tam URL'ye dönüştürür.
    pub fn resolve(&self, href: &str) -> Result<Self, UrlError> {
        let href = href.trim();
        if href.is_empty() {
            return Err(UrlError::EmptyUrl);
        }

        // 1. Eğer link zaten "http://" veya "https://" gibi tam bir adres ise doğrudan ayrıştır
        if href.contains("://") {
            return Url::parse(href);
        }

        // 2. "//ornek.com/yol" şeklinde protokolsüz link ise mevcut protokolü ekle
        if href.starts_with("//") {
            return Url::parse(&format!("{}:{}", self.scheme, href));
        }

        let port_str = match self.port {
            Some(p) => format!(":{p}"),
            None => String::new(),
        };

        // 3. "/sayfa.html" şeklinde kök dizinden başlıyorsa host'un sonuna ekle
        if href.starts_with('/') {
            return Url::parse(&format!("{}://{}{}{}", self.scheme, self.host, port_str, href));
        }

        // 4. "Status.html" gibi bağıl bir dosya ise mevcut klasör yolunun sonuna ekle
        let base_dir = match self.path.rfind('/') {
            Some(idx) => &self.path[..=idx],
            None => "/",
        };
        Url::parse(&format!(
            "{}://{}{}{}{}",
            self.scheme, self.host, port_str, base_dir, href
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tam_url_dograma() {
        let sonuc = Url::parse("https://anka.dev:8080/indir?surum=1#windows");
        let url = sonuc.unwrap();

        assert_eq!(url.scheme, "https");
        assert_eq!(url.host, "anka.dev");
        assert_eq!(url.port, Some(8080));
        assert_eq!(url.path, "/indir");
        assert_eq!(url.query, Some("surum=1".to_string()));
        assert_eq!(url.fragment, Some("windows".to_string()));
    }

    #[test]
    fn test_hatali_url_yakalama() {
        assert_eq!(Url::parse(""), Err(UrlError::EmptyUrl));
        assert_eq!(Url::parse("anka.dev"), Err(UrlError::MissingScheme));
        assert_eq!(Url::parse("http://"), Err(UrlError::MissingHost));
        assert_eq!(Url::parse("http://anka.dev:elma"), Err(UrlError::InvalidPort));
    }

    #[test]
    fn test_varsayilan_portlar() {
        // http için varsayılan port 80 olmalı
        let http_url = Url::parse("http://anka.dev").unwrap();
        assert_eq!(http_url.port, None);
        assert_eq!(http_url.effective_port(), Some(80));

        // https için varsayılan port 443 olmalı
        let https_url = Url::parse("https://anka.dev").unwrap();
        assert_eq!(https_url.port, None);
        assert_eq!(https_url.effective_port(), Some(443));

        // Elle port yazılmışsa (8080) onu ezmemeli, 8080 vermeli
        let custom_url = Url::parse("http://anka.dev:8080").unwrap();
        assert_eq!(custom_url.port, Some(8080));
        assert_eq!(custom_url.effective_port(), Some(8080));
    }

    #[test]
    fn test_bagil_url_cozumleme() {
        let base = Url::parse("http://info.cern.ch/hypertext/WWW/TheProject.html").unwrap();

        // Kök dizinden başlayan link
        let kok = base.resolve("/index.html").unwrap();
        assert_eq!(kok.host, "info.cern.ch");
        assert_eq!(kok.path, "/index.html");

        // Aynı klasördeki bağıl link
        let bagil = base.resolve("Status.html").unwrap();
        assert_eq!(bagil.path, "/hypertext/WWW/Status.html");

        // Tam URL linki
        let tam = base.resolve("https://example.com/test").unwrap();
        assert_eq!(tam.scheme, "https");
        assert_eq!(tam.host, "example.com");
    }
}
