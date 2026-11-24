// Anka Browser - Sıfır Bağımlılıklı Yerel Linux HTTPS / TLS 1.3 İstemcisi (OpenSSL)
// Tüm Linux dağıtımlarında (Milis, Ubuntu, Debian, Fedora, Arch, Pardus) yerleşik gelen
// `libssl.so` sistem kütüphanesini doğrudan FFI üzerinden kullanır.

#![cfg(not(windows))]

use std::ffi::{CString, c_char, c_int, c_long, c_void};
use std::net::TcpStream;
use std::os::fd::AsRawFd;

use anka_url::Url;

use crate::{HttpResponse, NetError, parse_http_response_bytes};

// OpenSSL'in C dilindeki opak (iç yapısı gizli) işaretçi tipleri:
type SslMethod = c_void;
type SslCtx = c_void;
type Ssl = c_void;

// OpenSSL SNI (Server Name Indication) sabitleri:
// Tek bir IP adresinde birden fazla HTTPS sitesi barındırıldığında (örn: Cloudflare, GitHub, Wikipedia),
// sunucuya "Ben hangi alan adına (hostname) bağlanmak istiyorum?" bilgisini TLS el sıkışması (handshake)
// sırasında iletmek için `SSL_ctrl` fonksiyonuna gönderilen standart OpenSSL komut kodları:
const SSL_CTRL_SET_TLSEXT_HOSTNAME: c_int = 55;
const TLSEXT_NAMETYPE_HOST_NAME: c_long = 0;

// Sistemde kurulu olan `libssl.so` kütüphanesine doğrudan bağlanıyoruz (Sıfır harici crate!)
#[link(name = "ssl")]
unsafe extern "C" {
    // 1. Modern TLS (TLS 1.2 ve TLS 1.3) istemci yöntemini döndürür
    fn TLS_client_method() -> *const SslMethod;

    // 2. Yeni bir TLS bağlamı (Context - ayar ve sertifika havuzu) oluşturur ve siler
    fn SSL_CTX_new(method: *const SslMethod) -> *mut SslCtx;
    fn SSL_CTX_free(ctx: *mut SslCtx);

    // 3. Tek bir bağlantı için yeni bir SSL oturum nesnesi oluşturur ve siler
    fn SSL_new(ctx: *mut SslCtx) -> *mut Ssl;
    fn SSL_free(ssl: *mut Ssl);

    // 4. Rust'ın açtığı TCP soketini (File Descriptor - fd) OpenSSL'e bağlar
    fn SSL_set_fd(ssl: *mut Ssl, fd: c_int) -> c_int;

    // 5. SNI (Server Name Indication) alan adını ayarlamak için kullanılan kontrol fonksiyonu
    fn SSL_ctrl(ssl: *mut Ssl, cmd: c_int, larg: c_long, parg: *mut c_void) -> c_long;

    // 6. Sunucuyla TLS El Sıkışmasını (Handshake - şifreleme anahtarı değişimi) başlatır
    fn SSL_connect(ssl: *mut Ssl) -> c_int;

    // 7. Şifreli tünelden veri gönderir (HTTP GET isteği) ve veri okur (HTTP yanıtı)
    fn SSL_write(ssl: *mut Ssl, buf: *const c_void, num: c_int) -> c_int;
    fn SSL_read(ssl: *mut Ssl, buf: *mut c_void, num: c_int) -> c_int;

    // 8. Şifreli bağlantıyı güvenli şekilde kapatır
    fn SSL_shutdown(ssl: *mut Ssl) -> c_int;
}

/// BELLEK GÜVENLİĞİ (RAII Guard):
/// Fonksiyondan nerede çıkılırsa çıkılsın (hata durumunda bile), OpenSSL'in C belleğinde
/// ayırdığı `SSL` ve `SSL_CTX` nesnelerini otomatik olarak temizleyen koruyucu yapı.
struct TlsGuard {
    ssl: *mut Ssl,
    ctx: *mut SslCtx,
}

impl Drop for TlsGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.ssl.is_null() {
                SSL_shutdown(self.ssl);
                SSL_free(self.ssl);
            }
            if !self.ctx.is_null() {
                SSL_CTX_free(self.ctx);
            }
        }
    }
}

/// Verilen `https://` adresine Linux'un yerleşik OpenSSL (`libssl`) motoru üzerinden
/// tek bir güvenli HTTP/1.1 GET isteği atar ve yanıtı `HttpResponse` olarak döndürür.
pub fn fetch_https_once(url: &Url) -> Result<HttpResponse, NetError> {
    // 1. ADIM: Kapı numarasını (port) al (https için varsayılan: 443)
    let port = url.effective_port().ok_or(NetError::MissingPort)?;

    // 2. ADIM: İstek yolunu hazırla (Varsa ?query kısmını ekle)
    let request_path = match &url.query {
        Some(q) => format!("{}?{}", url.path, q),
        None => url.path.clone(),
    };

    // 3. ADIM: İşletim sisteminden standart TCP soketini aç (Henüz şifresiz ham hat)
    let stream = TcpStream::connect((url.host.as_str(), port))?;

    // C diline göndereceğimiz alan adını (örn: "example.com\0") sonu sıfır bitli CString yap
    let c_host = CString::new(url.host.as_str()).map_err(|_| NetError::InvalidResponse)?;

    unsafe {
        // 4. ADIM: OpenSSL TLS motorunu başlat
        let method = TLS_client_method();
        if method.is_null() {
            return Err(NetError::InvalidResponse);
        }

        let ctx = SSL_CTX_new(method);
        if ctx.is_null() {
            return Err(NetError::InvalidResponse);
        }

        let ssl = SSL_new(ctx);
        if ssl.is_null() {
            SSL_CTX_free(ctx);
            return Err(NetError::InvalidResponse);
        }

        // Bellek sızıntısını önlemek için `TlsGuard` koruyucusuna teslim et
        let guard = TlsGuard { ssl, ctx };

        // 5. ADIM: Rust'ın TCP soket numarasını (`as_raw_fd()`) OpenSSL'e devret!
        if SSL_set_fd(guard.ssl, stream.as_raw_fd()) != 1 {
            return Err(NetError::InvalidResponse);
        }

        // 6. ADIM: SNI (Server Name Indication) ayarla!
        // Modern HTTPS sunucuları (Cloudflare, Vikipedi, GitHub vb.) SNI olmadan bağlantıyı reddeder.
        let sni_ok = SSL_ctrl(
            guard.ssl,
            SSL_CTRL_SET_TLSEXT_HOSTNAME,
            TLSEXT_NAMETYPE_HOST_NAME,
            c_host.as_ptr() as *mut c_char as *mut c_void,
        );
        if sni_ok == 0 {
            return Err(NetError::InvalidResponse);
        }

        // 7. ADIM: TLS El Sıkışmasını (Handshake) gerçekleştir!
        // Bu satırda TLS 1.3 şifreleme anahtarları sunucuyla anlaşılır ve hat şifrelenir.
        if SSL_connect(guard.ssl) != 1 {
            return Err(NetError::InvalidResponse);
        }

        // 8. ADIM: Şifreli tünelin içinden HTTP/1.1 GET mektubumuzu gönder!
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: AnkaBrowser/0.2.3 (Linux; TelekEngine)\r\nAccept: text/html,application/xhtml+xml,text/css,*/*\r\nConnection: close\r\n\r\n",
            request_path, url.host
        );

        let req_bytes = request.as_bytes();
        let mut total_written = 0;
        while total_written < req_bytes.len() {
            let remaining = &req_bytes[total_written..];
            let written = SSL_write(
                guard.ssl,
                remaining.as_ptr().cast::<c_void>(),
                remaining.len() as c_int,
            );
            if written <= 0 {
                return Err(NetError::InvalidResponse);
            }
            total_written += written as usize;
        }

        // 9. ADIM: Sunucudan gelen şifreli yanıtı parça parça çözerek (`SSL_read`) oku!
        let mut raw_bytes = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = SSL_read(
                guard.ssl,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            );
            if n <= 0 {
                break;
            }
            raw_bytes.extend_from_slice(&buf[..n as usize]);
        }

        // 10. ADIM: İndirilen ham baytları bozmadan HTTP doğrayıcımıza (`parse_http_response_bytes`) ver!
        // Böylece hem HTML/CSS metinleri hem de PNG/JPEG/BMP ikili görselleri tek bir baytı
        // bile bozulmadan ayrıştırılır!
        parse_http_response_bytes(&raw_bytes)
    }
}
