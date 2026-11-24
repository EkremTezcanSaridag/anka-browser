// Anka Browser - Ağ Katmanı (HTTP ve TCP İstemcisi)

use std::io::{Read, Write};
use std::net::TcpStream;

// Bir önceki modülde yazdığımız Url paketini buraya çağırıyoruz.
// Böylece anka-net, anka-url'in parçaladığı adresleri anlayabilir.
use anka_url::Url;

#[cfg(windows)]
mod winhttp_tls;

#[cfg(not(windows))]
mod linux_tls;

/// Tarayıcımız internete bağlanırken veya veri indirirken
/// karşılaşabileceği tüm olası hata durumlarının listesi:
#[derive(Debug)]
pub enum NetError {
    // 1. İşletim sistemi veya soket hatası:
    // İnternet kesilirse, sunucu kapalıysa veya kablo koparsa
    // Windows'un bize fırlattığı ham sistem hatasını (std::io::Error) burada saklarız.
    IoError(std::io::Error),

    // 2. Desteklenmeyen protokol hatası:
    // Kullanıcı "ftp://..." gibi henüz desteklemediğimiz
    // bir protokolle gelirse bu hatayı fırlatırız.
    UnsupportedScheme(String),

    // 3. Port numarası bulunamadı hatası:
    // Adreste port yazmıyorsa ve varsayılan bir port da tespit edilemediyse.
    MissingPort,

    // 4. Geçersiz web yanıtı hatası:
    // Sunucuya bağlandık ama sunucu bize HTTP standardına uymayan
    // anlamsız/bozuk baytlar gönderdiyse bu hatayı veririz.
    InvalidResponse,

    // 5. Sonsuz yönlendirme döngüsü koruması (5'ten fazla 301/302 gelirse dur!)
    TooManyRedirects,
}



/// OTOMATİK TERCÜMAN (Type Conversion):
/// Windows işletim sistemi hata verdiğinde bize "std::io::Error" döner.
/// Bu küçük blok sayesinde Rust, işletim sisteminden gelen o hatayı
/// hiç bizi uğraştırmadan otomatik olarak bizim "NetError::IoError" kutumuza dönüştürür.
impl From<std::io::Error> for NetError {
    fn from(err: std::io::Error) -> Self {
        NetError::IoError(err)
    }
}

/// Sunucudan bize dönen HTTP yanıtını tutan veri paketi
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    // HTTP yanıtının durum kodu (örn: 200, 404, 500)
    pub status_code: u16,
    // Sunucunun gönderdiği başlıklar (örn: Content-Type, Content-Length)
    pub headers: Vec<(String, String)>,
    // Sunucunun gönderdiği HTML metni
    pub body: String,
}

impl HttpResponse {
    /// Zarfın (`self.headers`) içinde aradığımız başlığı (örn: "location") bulur.
    /// - `&self`: Bu `HttpResponse` paketinin kendi verilerine sadece okuma amaçlı bakar.
    /// - `name: &str`: Aradığımız başlığın adı (örn: `"location"`).
    /// - `-> Option<&str>`: Başlığı bulursa `Some("http://...")`, bulamazsa `None` (boş) döner.
    pub fn get_header(&self, name: &str) -> Option<&str> {
        // Zarfın içindeki her (Anahtar, Değer) çiftini sırayla tek tek gez:
        // Örn: 1. tur -> key = "Content-Type", value = "text/html"
        //      2. tur -> key = "Location",     value = "http://www.google.com/"
        for (key, value) in &self.headers {
            // `eq_ignore_ascii_case`: Büyük/küçük harf farkını yoksayarak karşılaştırır!
            // Yani "Location" == "location" veya "LOCATION" == "location" -> DOĞRU (true) verir.
            if key.eq_ignore_ascii_case(name) {
                // Aradığımız başlığı bulduk! Değerini (`Some(value)`) hemen döndür ve döngüden çık.
                return Some(value);
            }
        }
        // Tüm başlıkları gezdik ama aradığımız başlık zarfta yokmuş -> Boş (`None`) dön.
        None
    }
}

/// Verilen `Url` adresine bağlanır (`http` veya `https`) ve sunucu `301 / 302 / 303 / 307 / 308`
/// yönlendirme yanıtı dönerse `Location:` başlığını otomatik takip eder (en fazla 5 kez).
pub fn fetch(url: &Url) -> Result<HttpResponse, NetError> {
    const MAX_REDIRECTS: usize = 5;
    let mut current_url = url.clone();

    for _ in 0..=MAX_REDIRECTS {
        // Protokole göre tek bir HTTP/1.1 veya HTTPS/TLS 1.3 isteği at:
        let response = if current_url.scheme == "https" {
            #[cfg(windows)]
            {
                return winhttp_tls::fetch_https(&current_url);
            }
            #[cfg(not(windows))]
            {
                linux_tls::fetch_https_once(&current_url)?
            }
        } else {
            fetch_once(&current_url)?
        };

        // Sunucu "Bu sayfa taşındı (301/302/307/308)" dediyse ve zarfta "Location" varsa:
        let is_redirect = matches!(response.status_code, 301 | 302 | 303 | 307 | 308);
        if is_redirect && let Some(location) = response.get_header("location") {
            // Yeni adresi (kısa/bağıl bile olsa) `anka-url::Url::resolve` ile tam adrese çevir
            current_url = current_url
                .resolve(location)
                .map_err(|_| NetError::InvalidResponse)?;
            // Döngünün başına dönüp yeni adrese bağlan!
            continue;
        }

        // Yönlendirme yoksa (örn: 200 OK veya 404), cevabı doğrudan teslim et!
        return Ok(response);
    }

    // 5 yönlendirmeyi aştıysak sonsuz döngüye girmemek için dur!
    Err(NetError::TooManyRedirects)
}

/// Verilen `Url` adresine TCP üzerinden tek bir HTTP/1.1 GET isteği atar
/// ve sunucudan dönen yanıtı `HttpResponse` paketi olarak döndürür.
fn fetch_once(url: &Url) -> Result<HttpResponse, NetError> {
    // 1. ADIM: Protokol kontrolü (Şimdilik düz "http" bağlantısını destekliyoruz)
    if url.scheme != "http" {
        return Err(NetError::UnsupportedScheme(url.scheme.clone()));
    }

    // 2. ADIM: Kapı numarasını (port) öğren (Örn: http için 80)
    let port = match url.effective_port() {
        Some(p) => p,
        None => return Err(NetError::MissingPort),
    };

    // 3. ADIM: Sunucudan istenecek sayfa yolunu hazırla (Varsa ?query kısmını da ekle)
    let request_path = match &url.query {
        Some(q) => format!("{}?{}", url.path, q),
        None => url.path.clone(),
    };

    // 4. ADIM: İşletim sisteminden TCP telefon hattını aç ve sunucuya bağlan!
    // Sondaki `?` işareti: "Bağlanamazsan hatayı tercümana verip hemen çık" demektir.
    let mut stream = TcpStream::connect((url.host.as_str(), port))?;

    // 5. ADIM: Sunucuya göndereceğimiz HTTP/1.1 mektubunu hazırla
    // Her satır internet standardı gereği "\r\n" ile biter, en sonda boş bir "\r\n" olur.
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: AnkaBrowser/0.1\r\nConnection: close\r\n\r\n",
        request_path, url.host
    );

    // Mektubu internet kablosundan karşı sunucuya gönder!
    stream.write_all(request.as_bytes())?;

    // 6. ADIM: Sunucunun gönderdiği cevabı sonuna kadar oku
    let mut raw_response = String::new();
    stream.read_to_string(&mut raw_response)?;

    // 7. ADIM: Gelen ham cevabı doğrayıp HttpResponse kutusuna yerleştir
    parse_http_response(&raw_response)
}

/// Sunucudan gelen ham HTTP metnini doğrayarak `HttpResponse` paketine çevirir.
/// Örnek girdi:
/// "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html>Merhaba</html>"
pub fn parse_http_response(raw: &str) -> Result<HttpResponse, NetError> {
    // 1. ADIM: Yanıtı boş satırdan ("\r\n\r\n") ikiye kes:
    // Üst kısım = Durum satırı ve başlıklar (headers)
    // Alt kısım = HTML gövdesi (body)
    let (header_section, body) = match raw.split_once("\r\n\r\n") {
        Some((h, b)) => (h, b),
        None => return Err(NetError::InvalidResponse),
    };

    // Üst kısmı satır satır okumak için bir okuyucu (iterator) başlat
    let mut lines = header_section.lines();

    // 2. ADIM: İlk satırı oku (Örn: "HTTP/1.1 200 OK")
    let status_line = match lines.next() {
        Some(line) => line,
        None => return Err(NetError::InvalidResponse),
    };

    // İlk satırı boşluklardan böl: ["HTTP/1.1", "200", "OK"]
    // 2. kelime olan "200" sayısını al ve u16 tam sayısına çevir
    let mut parts = status_line.split_whitespace();
    let _http_version = parts.next().ok_or(NetError::InvalidResponse)?;
    let status_code_str = parts.next().ok_or(NetError::InvalidResponse)?;
    let status_code = status_code_str
        .parse::<u16>()
        .map_err(|_| NetError::InvalidResponse)?;

    // 3. ADIM: Kalan satırlardaki başlıkları ("Content-Type: text/html") tek tek ayır
    let mut headers = Vec::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }
    }

    // 4. ADIM: Eğer sunucu veriyi parça parça (`Transfer-Encoding: chunked`) gönderdiyse birleştir!
    let is_chunked = headers.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("transfer-encoding") && v.to_lowercase().contains("chunked")
    });
    let final_body = if is_chunked {
        decode_chunked(body)?
    } else {
        body.to_string()
    };

    // 5. ADIM: Hazırladığımız HttpResponse kutusunu döndür!
    Ok(HttpResponse {
        status_code,
        headers,
        body: final_body,
    })
}

/// `Transfer-Encoding: chunked` formatında gelen HTTP gövdesini birleştirir.
/// Örnek: `"7\r\nMerhaba\r\n5\r\n Anka\r\n0\r\n\r\n"` -> `"Merhaba Anka"`
pub fn decode_chunked(raw_body: &str) -> Result<String, NetError> {
    let mut result = String::new();
    let mut kalan = raw_body;

    loop {
        let Some((size_line, rest)) = kalan.split_once("\r\n") else {
            return Err(NetError::InvalidResponse);
        };
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let chunk_size = usize::from_str_radix(size_hex, 16).map_err(|_| NetError::InvalidResponse)?;

        // 0 uzunluğundaki parça verinin bittiğini gösterir
        if chunk_size == 0 {
            break;
        }

        if rest.len() < chunk_size {
            return Err(NetError::InvalidResponse);
        }

        result.push_str(&rest[..chunk_size]);
        kalan = rest[chunk_size..]
            .strip_prefix("\r\n")
            .unwrap_or(&rest[chunk_size..]);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_yanit_dograma() {
        let sahte_sunucu_yaniti = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nServer: AnkaTest\r\n\r\n<html><body><h1>Merhaba Anka!</h1></body></html>";

        let yanit = parse_http_response(sahte_sunucu_yaniti).unwrap();

        assert_eq!(yanit.status_code, 200);
        assert_eq!(yanit.headers.len(), 2);
        assert_eq!(
            yanit.headers[0],
            ("Content-Type".to_string(), "text/html".to_string())
        );
        assert_eq!(
            yanit.body,
            "<html><body><h1>Merhaba Anka!</h1></body></html>"
        );
    }

    #[test]
    fn test_desteklenmeyen_protokol_yakalama() {
        let ftp_url = Url::parse("ftp://ornek.com/dosya.zip").unwrap();
        let sonuc = fetch(&ftp_url);

        assert!(matches!(sonuc, Err(NetError::UnsupportedScheme(_))));
    }

    #[test]
    fn test_location_basligi_okuma() {
        let yonlendirme_yaniti = "HTTP/1.1 301 Moved Permanently\r\nLOCATION: http://www.google.com/\r\n\r\nMoved";
        let yanit = parse_http_response(yonlendirme_yaniti).unwrap();

        assert_eq!(yanit.status_code, 301);
        assert_eq!(
            yanit.get_header("location"),
            Some("http://www.google.com/")
        );
    }

    #[test]
    fn test_chunked_yanit_birlestirme() {
        let chunked_yanit = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n7\r\nMerhaba\r\n5\r\n Anka\r\n0\r\n\r\n";
        let yanit = parse_http_response(chunked_yanit).unwrap();

        assert_eq!(yanit.status_code, 200);
        assert_eq!(yanit.body, "Merhaba Anka");
    }
}
