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
const WINHTTP_QUERY_SET_COOKIE: DWORD = 43;
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
/// üzerinden güvenli `GET` veya `POST` isteği kurar; çerez gönderir ve `Set-Cookie` başlıklarını okur.
pub fn request_https(
    url: &Url,
    method: &str,
    cookie_header: Option<&str>,
    post_body: Option<&str>,
) -> Result<HttpResponse, NetError> {
    let port = url.effective_port().ok_or(NetError::MissingPort)?;
    let path_and_query = match &url.query {
        Some(q) => format!("{}?{}", url.path, q),
        None => url.path.clone(),
    };

    unsafe {
        let agent = to_wide("AnkaBrowser/0.3.1");
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

        let verb_w = to_wide(method);
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

        let mut extra_headers = String::new();
        if let Some(cookie) = cookie_header
            && !cookie.is_empty()
        {
            extra_headers.push_str(&format!("Cookie: {cookie}\r\n"));
        }
        if post_body.is_some() {
            extra_headers.push_str("Content-Type: application/x-www-form-urlencoded\r\n");
        }
        let extra_w = if extra_headers.is_empty() {
            None
        } else {
            Some(to_wide(&extra_headers))
        };
        let (hdr_ptr, hdr_len) = match &extra_w {
            Some(w) => (w.as_ptr(), (w.len() - 1) as DWORD),
            None => (null(), 0),
        };
        let mut body_buf = post_body.map(|b| b.as_bytes().to_vec()).unwrap_or_default();
        let (body_ptr, body_len) = if body_buf.is_empty() {
            (null_mut(), 0)
        } else {
            (body_buf.as_mut_ptr().cast::<c_void>(), body_buf.len() as DWORD)
        };

        let sent = WinHttpSendRequest(
            h_request,
            hdr_ptr,
            hdr_len,
            body_ptr,
            body_len,
            body_len,
            0,
        );
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

        let mut headers = vec![("X-Anka-Transport".to_string(), "HTTPS/TLS".to_string())];
        let mut cookie_idx: DWORD = 0;
        loop {
            let mut buf_w = [0u16; 1024];
            let mut buf_bytes: DWORD = (buf_w.len() * 2) as DWORD;
            let ok = WinHttpQueryHeaders(
                h_request,
                WINHTTP_QUERY_SET_COOKIE,
                null(),
                buf_w.as_mut_ptr().cast::<c_void>(),
                &mut buf_bytes,
                &mut cookie_idx,
            );
            if ok == 0 {
                break;
            }
            let wide_len = (buf_bytes as usize) / 2;
            let val = String::from_utf16_lossy(&buf_w[..wide_len.min(buf_w.len())])
                .trim_end_matches('\0')
                .trim()
                .to_string();
            if !val.is_empty() {
                headers.push(("Set-Cookie".to_string(), val));
            }
        }

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
            headers,
            body,
            raw_body: body_bytes,
        })
    }
}
