// Anka Browser - Sıfır Bağımlılıklı Yerel Windows HTTPS / TLS 1.3 İstemcisi (WinHTTP)
// Rust'ın kendi içinde gelen `libwinhttp.a` sistem kütüphanesini kullanır.

#![cfg(windows)]
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::ptr::{null, null_mut};

use anka_url::Url;

use crate::{HttpResponse, NetError};

type HINTERNET = *mut c_void;
type DWORD = u32;
type WORD = u16;
type BOOL = i32;

const WINHTTP_ACCESS_TYPE_DEFAULT_PROXY: DWORD = 0;
const WINHTTP_FLAG_SECURE: DWORD = 0x0080_0000;
const WINHTTP_QUERY_STATUS_CODE: DWORD = 19;
const WINHTTP_QUERY_FLAG_NUMBER: DWORD = 0x2000_0000;

#[link(name = "winhttp")]
unsafe extern "system" {
    fn WinHttpOpen(
        pszAgentW: *const u16,
        dwAccessType: DWORD,
        pszProxyW: *const u16,
        pszProxyBypassW: *const u16,
        dwFlags: DWORD,
    ) -> HINTERNET;
    fn WinHttpConnect(
        hSession: HINTERNET,
        pswzServerName: *const u16,
        nServerPort: WORD,
        dwReserved: DWORD,
    ) -> HINTERNET;
    fn WinHttpOpenRequest(
        hConnect: HINTERNET,
        pwszVerb: *const u16,
        pwszObjectName: *const u16,
        pwszVersion: *const u16,
        pwszReferrer: *const u16,
        ppwszAcceptTypes: *const *const u16,
        dwFlags: DWORD,
    ) -> HINTERNET;
    fn WinHttpSendRequest(
        hRequest: HINTERNET,
        lpszHeaders: *const u16,
        dwHeadersLength: DWORD,
        lpOptional: *mut c_void,
        dwOptionalLength: DWORD,
        dwTotalLength: DWORD,
        dwContext: usize,
    ) -> BOOL;
    fn WinHttpReceiveResponse(hRequest: HINTERNET, lpReserved: *mut c_void) -> BOOL;
    fn WinHttpQueryHeaders(
        hRequest: HINTERNET,
        dwInfoLevel: DWORD,
        pwszName: *const u16,
        lpBuffer: *mut c_void,
        lpdwBufferLength: *mut DWORD,
        lpdwIndex: *mut DWORD,
    ) -> BOOL;
    fn WinHttpQueryDataAvailable(hRequest: HINTERNET, lpdwNumberOfBytesAvailable: *mut DWORD)
        -> BOOL;
    fn WinHttpReadData(
        hRequest: HINTERNET,
        lpBuffer: *mut c_void,
        dwNumberOfBytesToRead: DWORD,
        lpdwNumberOfBytesRead: *mut DWORD,
    ) -> BOOL;
    fn WinHttpCloseHandle(hInternet: HINTERNET) -> BOOL;
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Verilen `https://` adresine Windows'un yerleşik TLS 1.3 motoru (`winhttp`)
/// üzerinden güvenli bağlantı kurar ve sayfanın HTML içeriğini indirir.
pub fn fetch_https(url: &Url) -> Result<HttpResponse, NetError> {
    let port = url.effective_port().ok_or(NetError::MissingPort)?;
    let path_and_query = match &url.query {
        Some(q) => format!("{}?{}", url.path, q),
        None => url.path.clone(),
    };

    unsafe {
        let agent = to_wide("AnkaBrowser/0.2");
        let h_session = WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            null(),
            null(),
            0,
        );
        if h_session.is_null() {
            return Err(NetError::InvalidResponse);
        }

        let host_w = to_wide(&url.host);
        let h_connect = WinHttpConnect(h_session, host_w.as_ptr(), port, 0);
        if h_connect.is_null() {
            WinHttpCloseHandle(h_session);
            return Err(NetError::InvalidResponse);
        }

        let verb_w = to_wide("GET");
        let path_w = to_wide(&path_and_query);
        let h_request = WinHttpOpenRequest(
            h_connect,
            verb_w.as_ptr(),
            path_w.as_ptr(),
            null(),
            null(),
            null(),
            WINHTTP_FLAG_SECURE,
        );
        if h_request.is_null() {
            WinHttpCloseHandle(h_connect);
            WinHttpCloseHandle(h_session);
            return Err(NetError::InvalidResponse);
        }

        let sent = WinHttpSendRequest(h_request, null(), 0, null_mut(), 0, 0, 0);
        if sent == 0 || WinHttpReceiveResponse(h_request, null_mut()) == 0 {
            WinHttpCloseHandle(h_request);
            WinHttpCloseHandle(h_connect);
            WinHttpCloseHandle(h_session);
            return Err(NetError::InvalidResponse);
        }

        // Durum kodunu (örn: 200) oku
        let mut status_code_u32: DWORD = 200;
        let mut status_size: DWORD = std::mem::size_of::<DWORD>() as DWORD;
        WinHttpQueryHeaders(
            h_request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            (&mut status_code_u32 as *mut DWORD).cast::<c_void>(),
            &mut status_size,
            null_mut(),
        );

        // Yanıt gövdesini (HTML baytlarını) parça parça oku
        let mut body_bytes = Vec::new();
        loop {
            let mut available: DWORD = 0;
            if WinHttpQueryDataAvailable(h_request, &mut available) == 0 || available == 0 {
                break;
            }
            let mut chunk = vec![0u8; available as usize];
            let mut read: DWORD = 0;
            if WinHttpReadData(
                h_request,
                chunk.as_mut_ptr().cast::<c_void>(),
                available,
                &mut read,
            ) == 0
                || read == 0
            {
                break;
            }
            body_bytes.extend_from_slice(&chunk[..read as usize]);
        }

        WinHttpCloseHandle(h_request);
        WinHttpCloseHandle(h_connect);
        WinHttpCloseHandle(h_session);

        let body = String::from_utf8_lossy(&body_bytes).into_owned();
        Ok(HttpResponse {
            status_code: status_code_u32 as u16,
            headers: vec![("X-Anka-Transport".to_string(), "HTTPS/TLS".to_string())],
            body,
            raw_body: body_bytes,
        })
    }
}
