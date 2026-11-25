// Anka Browser - Sıfır Bağımlılıklı Yerel Linux HTTPS / TLS 1.3 İstemcisi (OpenSSL + dlopen)
// Derleme sırasında `libssl-dev` paketi gerektirmez! Çalışma zamanında sistemde zaten yüklü
// bulunan `libssl.so.3` (veya `libssl.so.1.1` / `libssl.so`) kütüphanesini `dlopen` + `dlsym`
// ile dinamik olarak yükler. Tüm Linux dağıtımlarında ve GitHub Actions CI'da sıfır sorunla çalışır.

#![cfg(not(windows))]

use std::ffi::{CStr, CString, c_char, c_int, c_long, c_void};
use std::net::TcpStream;
use std::os::fd::AsRawFd;
use std::sync::OnceLock;

use anka_url::Url;

use crate::{HttpResponse, NetError, parse_http_response_bytes};

// OpenSSL'in C dilindeki opak işaretçi tipleri:
type SslMethod = c_void;
type SslCtx = c_void;
type Ssl = c_void;

// OpenSSL SNI (Server Name Indication) sabitleri:
const SSL_CTRL_SET_TLSEXT_HOSTNAME: c_int = 55;
const TLSEXT_NAMETYPE_HOST_NAME: c_long = 0;

// POSIX dinamik kütüphane yükleme sabitleri (`dlfcn.h`):
const RTLD_NOW: c_int = 2;

// Linux/Unix'in standart dinamik bağlayıcı (`libc` / `libdl`) fonksiyonları — her sistemde yerleşiktir!
unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// Çalışma zamanında `libssl.so` içinden yüklenen OpenSSL C fonksiyon işaretçileri tablosu:
struct OpenSslApi {
    tls_client_method: unsafe extern "C" fn() -> *const SslMethod,
    ssl_ctx_new: unsafe extern "C" fn(*const SslMethod) -> *mut SslCtx,
    ssl_ctx_free: unsafe extern "C" fn(*mut SslCtx),
    ssl_new: unsafe extern "C" fn(*mut SslCtx) -> *mut Ssl,
    ssl_free: unsafe extern "C" fn(*mut Ssl),
    ssl_set_fd: unsafe extern "C" fn(*mut Ssl, c_int) -> c_int,
    ssl_ctrl: unsafe extern "C" fn(*mut Ssl, c_int, c_long, *mut c_void) -> c_long,
    ssl_connect: unsafe extern "C" fn(*mut Ssl) -> c_int,
    ssl_write: unsafe extern "C" fn(*mut Ssl, *const c_void, c_int) -> c_int,
    ssl_read: unsafe extern "C" fn(*mut Ssl, *mut c_void, c_int) -> c_int,
    ssl_shutdown: unsafe extern "C" fn(*mut Ssl) -> c_int,
}

unsafe impl Send for OpenSslApi {}
unsafe impl Sync for OpenSslApi {}

static OPENSSL_API: OnceLock<Option<OpenSslApi>> = OnceLock::new();

unsafe fn load_sym<T>(handle: *mut c_void, name: &CStr) -> Option<T> {
    let ptr = unsafe { dlsym(handle, name.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

fn openssl_api() -> Option<&'static OpenSslApi> {
    OPENSSL_API
        .get_or_init(|| unsafe {
            // Farklı Linux dağıtımlarındaki OpenSSL sürüm adlarını sırayla dene:
            let candidates: &[&CStr] = &[c"libssl.so.3", c"libssl.so.1.1", c"libssl.so"];
            let mut handle = std::ptr::null_mut();
            for name in candidates {
                handle = dlopen(name.as_ptr(), RTLD_NOW);
                if !handle.is_null() {
                    break;
                }
            }
            if handle.is_null() {
                return None;
            }

            Some(OpenSslApi {
                tls_client_method: load_sym(handle, c"TLS_client_method")?,
                ssl_ctx_new: load_sym(handle, c"SSL_CTX_new")?,
                ssl_ctx_free: load_sym(handle, c"SSL_CTX_free")?,
                ssl_new: load_sym(handle, c"SSL_new")?,
                ssl_free: load_sym(handle, c"SSL_free")?,
                ssl_set_fd: load_sym(handle, c"SSL_set_fd")?,
                ssl_ctrl: load_sym(handle, c"SSL_ctrl")?,
                ssl_connect: load_sym(handle, c"SSL_connect")?,
                ssl_write: load_sym(handle, c"SSL_write")?,
                ssl_read: load_sym(handle, c"SSL_read")?,
                ssl_shutdown: load_sym(handle, c"SSL_shutdown")?,
            })
        })
        .as_ref()
}

/// BELLEK GÜVENLİĞİ (RAII Guard):
/// Fonksiyondan nerede çıkılırsa çıkılsın (hata durumunda bile), OpenSSL'in C belleğinde
/// ayırdığı `SSL` ve `SSL_CTX` nesnelerini otomatik olarak temizleyen koruyucu yapı.
struct TlsGuard {
    api: &'static OpenSslApi,
    ssl: *mut Ssl,
    ctx: *mut SslCtx,
}

impl Drop for TlsGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.ssl.is_null() {
                (self.api.ssl_shutdown)(self.ssl);
                (self.api.ssl_free)(self.ssl);
            }
            if !self.ctx.is_null() {
                (self.api.ssl_ctx_free)(self.ctx);
            }
        }
    }
}

/// Verilen `https://` adresine Linux'un yerleşik OpenSSL (`libssl.so`) motoru üzerinden
/// tek bir güvenli HTTP/1.1 GET isteği atar ve yanıtı `HttpResponse` olarak döndürür.
pub fn fetch_https_once(url: &Url) -> Result<HttpResponse, NetError> {
    let api = openssl_api().ok_or(NetError::InvalidResponse)?;

    // 1. ADIM: Kapı numarasını (port) al (https için varsayılan: 443)
    let port = url.effective_port().ok_or(NetError::MissingPort)?;

    // 2. ADIM: İstek yolunu hazırla (Varsa ?query kısmını ekle)
    let request_path = match &url.query {
        Some(q) => format!("{}?{}", url.path, q),
        None => url.path.clone(),
    };

    // 3. ADIM: İşletim sisteminden standart TCP soketini aç (Henüz şifresiz ham hat)
    let stream = TcpStream::connect((url.host.as_str(), port))?;
    let c_host = CString::new(url.host.as_str()).map_err(|_| NetError::InvalidResponse)?;

    unsafe {
        // 4. ADIM: OpenSSL TLS motorunu başlat
        let method = (api.tls_client_method)();
        if method.is_null() {
            return Err(NetError::InvalidResponse);
        }

        let ctx = (api.ssl_ctx_new)(method);
        if ctx.is_null() {
            return Err(NetError::InvalidResponse);
        }

        let ssl = (api.ssl_new)(ctx);
        if ssl.is_null() {
            (api.ssl_ctx_free)(ctx);
            return Err(NetError::InvalidResponse);
        }

        let guard = TlsGuard { api, ssl, ctx };

        // 5. ADIM: Rust'ın TCP soket numarasını (`as_raw_fd()`) OpenSSL'e devret!
        if (api.ssl_set_fd)(guard.ssl, stream.as_raw_fd()) != 1 {
            return Err(NetError::InvalidResponse);
        }

        // 6. ADIM: SNI (Server Name Indication) ayarla!
        let sni_ok = (api.ssl_ctrl)(
            guard.ssl,
            SSL_CTRL_SET_TLSEXT_HOSTNAME,
            TLSEXT_NAMETYPE_HOST_NAME,
            c_host.as_ptr() as *mut c_char as *mut c_void,
        );
        if sni_ok == 0 {
            return Err(NetError::InvalidResponse);
        }

        // 7. ADIM: TLS El Sıkışmasını (Handshake) gerçekleştir!
        if (api.ssl_connect)(guard.ssl) != 1 {
            return Err(NetError::InvalidResponse);
        }

        // 8. ADIM: Şifreli tünelin içinden HTTP/1.1 GET mektubumuzu gönder!
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: AnkaBrowser/0.2.6 (Linux; TelekEngine)\r\nAccept: text/html,application/xhtml+xml,text/css,image/*,*/*\r\nConnection: close\r\n\r\n",
            request_path, url.host
        );

        let req_bytes = request.as_bytes();
        let mut total_written = 0;
        while total_written < req_bytes.len() {
            let remaining = &req_bytes[total_written..];
            let written = (api.ssl_write)(
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
            let n = (api.ssl_read)(
                guard.ssl,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            );
            if n <= 0 {
                break;
            }
            raw_bytes.extend_from_slice(&buf[..n as usize]);
        }

        // 10. ADIM: Ham baytları bozmadan HTTP doğrayıcımıza (`parse_http_response_bytes`) ver!
        parse_http_response_bytes(&raw_bytes)
    }
}
