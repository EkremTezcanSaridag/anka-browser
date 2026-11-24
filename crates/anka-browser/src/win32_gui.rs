// Anka Browser - Sıfır Bağımlılıklı Yerel Windows Pencere ve GDI Çizim Motoru
// Rust'ın kendi içinde hazır gelen user32 ve gdi32 sistem kütüphanelerini doğrudan kullanır!

#![cfg(windows)]
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::Mutex;

use crate::browser_state::BrowserState;

type HWND = *mut c_void;
type HINSTANCE = *mut c_void;
type HICON = *mut c_void;
type HCURSOR = *mut c_void;
type HBRUSH = *mut c_void;
type HMENU = *mut c_void;
type HDC = *mut c_void;
type HFONT = *mut c_void;
type HGDIOBJ = *mut c_void;
type LPARAM = isize;
type WPARAM = usize;
type LRESULT = isize;
type ATOM = u16;

const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CHILD: u32 = 0x4000_0000;
const WS_BORDER: u32 = 0x0080_0000;
const ES_AUTOHSCROLL: u32 = 0x0080;
const BS_PUSHBUTTON: u32 = 0x0000;
const CW_USEDEFAULT: i32 = -2147483648;

const WM_CREATE: u32 = 0x0001;
const WM_DESTROY: u32 = 0x0002;
const WM_SIZE: u32 = 0x0005;
const WM_PAINT: u32 = 0x000F;
const WM_COMMAND: u32 = 0x0111;
const WM_SETFONT: u32 = 0x0030;
const WM_KEYDOWN: u32 = 0x0100;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_MOUSEWHEEL: u32 = 0x020A;

const VK_RETURN: usize = 0x0D;
const IDC_ARROW: *const u16 = 32512 as *const u16;
const IDC_HAND: *const u16 = 32649 as *const u16;
const TRANSPARENT: i32 = 1;
const FW_NORMAL: i32 = 400;
const FW_BOLD: i32 = 700;
const DEFAULT_CHARSET: u32 = 1;
const OUT_DEFAULT_PRECIS: u32 = 0;
const CLIP_DEFAULT_PRECIS: u32 = 0;
const CLEARTYPE_QUALITY: u32 = 5;
const DEFAULT_PITCH: u32 = 0;

// Buton ve Kontrol Kimlik Numaraları
const ID_URL_EDIT: usize = 101;
const ID_BTN_GO: usize = 102;
const ID_BTN_HOME: usize = 103;
const ID_BTN_CERN: usize = 104;
const ID_BTN_EXAMPLE: usize = 105;
const ID_BTN_BACK: usize = 106;
const ID_BTN_FORWARD: usize = 107;
const ID_BTN_RELOAD: usize = 108;

const TOOLBAR_HEIGHT: i32 = 48;
const STATUS_HEIGHT: i32 = 28;

#[repr(C)]
struct WNDCLASSW {
    style: u32,
    lpfnWndProc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: HINSTANCE,
    hIcon: HICON,
    hCursor: HCURSOR,
    hbrBackground: HBRUSH,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
}

#[repr(C)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MSG {
    hwnd: HWND,
    message: u32,
    wParam: WPARAM,
    lParam: LPARAM,
    time: u32,
    pt: POINT,
}

#[repr(C)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct PAINTSTRUCT {
    hdc: HDC,
    fErase: i32,
    rcPaint: RECT,
    fRestore: i32,
    fIncUpdate: i32,
    rgbReserved: [u8; 32],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(lpModuleName: *const u16) -> HINSTANCE;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(lpWndClass: *const WNDCLASSW) -> ATOM;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        x: i32,
        y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: HWND,
        hMenu: HMENU,
        hInstance: HINSTANCE,
        lpParam: *mut c_void,
    ) -> HWND;
    fn ShowWindow(hWnd: HWND, nCmdShow: i32) -> i32;
    fn UpdateWindow(hWnd: HWND) -> i32;
    fn GetMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> LRESULT;
    fn DefWindowProcW(hWnd: HWND, msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
    fn PostQuitMessage(nExitCode: i32);
    fn LoadCursorW(hInstance: HINSTANCE, lpCursorName: *const u16) -> HCURSOR;
    fn SetCursor(hCursor: HCURSOR) -> HCURSOR;
    fn BeginPaint(hWnd: HWND, lpPaint: *mut PAINTSTRUCT) -> HDC;
    fn EndPaint(hWnd: HWND, lpPaint: *const PAINTSTRUCT) -> i32;
    fn FillRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    fn FrameRect(hdc: HDC, lprc: *const RECT, hbr: HBRUSH) -> i32;
    fn GetClientRect(hWnd: HWND, lpRect: *mut RECT) -> i32;
    fn InvalidateRect(hWnd: HWND, lpRect: *const RECT, bErase: i32) -> i32;
    fn GetWindowTextW(hWnd: HWND, lpString: *mut u16, nMaxCount: i32) -> i32;
    fn SetWindowTextW(hWnd: HWND, lpString: *const u16) -> i32;
    fn MoveWindow(hWnd: HWND, x: i32, y: i32, nWidth: i32, nHeight: i32, bRepaint: i32) -> i32;
    fn SendMessageW(hWnd: HWND, msg: u32, wParam: WPARAM, lParam: LPARAM) -> LRESULT;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateSolidBrush(color: u32) -> HBRUSH;
    fn DeleteObject(ho: HGDIOBJ) -> i32;
    fn SetBkMode(hdc: HDC, mode: i32) -> i32;
    fn SetTextColor(hdc: HDC, color: u32) -> u32;
    fn TextOutW(hdc: HDC, x: i32, y: i32, lpString: *const u16, c: i32) -> i32;
    fn CreateFontW(
        cHeight: i32,
        cWidth: i32,
        cEscapement: i32,
        cOrientation: i32,
        cWeight: i32,
        bItalic: u32,
        bUnderline: u32,
        bStrikeOut: u32,
        iCharSet: u32,
        iOutPrecision: u32,
        iClipPrecision: u32,
        iQuality: u32,
        iPitchAndFamily: u32,
        pszFaceName: *const u16,
    ) -> HFONT;
    fn SelectObject(hdc: HDC, h: HGDIOBJ) -> HGDIOBJ;
}

struct PencereDurumu {
    url_edit: usize,
    btn_go: usize,
    state: BrowserState,
}

static DURUM: Mutex<Option<PencereDurumu>> = Mutex::new(None);

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

fn pencereyi_yenile(hwnd: HWND, st: &PencereDurumu) {
    let url_edit = st.url_edit as HWND;
    let wide_url = to_wide(&st.state.current_url);
    let wide_title = to_wide(&st.state.window_title());
    unsafe {
        SetWindowTextW(url_edit, wide_url.as_ptr());
        SetWindowTextW(hwnd, wide_title.as_ptr());
        InvalidateRect(hwnd, null(), 1);
    }
}

fn sayfayi_yukle_ve_yenile(hwnd: HWND, adres: &str, gecmise_ekle: bool) {
    let mut rc = RECT {
        left: 0,
        top: 0,
        right: 880,
        bottom: 640,
    };
    unsafe {
        GetClientRect(hwnd, &mut rc);
    }
    let viewport_w = (rc.right - rc.left).max(400) as u32;

    if let Ok(mut guard) = DURUM.lock()
        && let Some(st) = guard.as_mut()
    {
        st.state.viewport_width = viewport_w;
        st.state.navigate(adres, gecmise_ekle);
        pencereyi_yenile(hwnd, st);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            unsafe {
                let hinst = GetModuleHandleW(null());
                let ui_font = CreateFontW(
                    -15,
                    0,
                    0,
                    0,
                    FW_NORMAL,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS,
                    CLEARTYPE_QUALITY,
                    DEFAULT_PITCH,
                    to_wide("Segoe UI").as_ptr(),
                );

                // Gezinme Butonları: <- Geri, -> İleri, Yenile, Ana Sayfa, CERN, HTTPS Example
                let btn_back = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("<-").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    8,
                    10,
                    32,
                    28,
                    hwnd,
                    ID_BTN_BACK as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_back, WM_SETFONT, ui_font as WPARAM, 1);

                let btn_forward = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("->").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    44,
                    10,
                    32,
                    28,
                    hwnd,
                    ID_BTN_FORWARD as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_forward, WM_SETFONT, ui_font as WPARAM, 1);

                let btn_reload = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("Yenile").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    80,
                    10,
                    54,
                    28,
                    hwnd,
                    ID_BTN_RELOAD as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_reload, WM_SETFONT, ui_font as WPARAM, 1);

                let btn_home = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("Ana Sayfa").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    138,
                    10,
                    76,
                    28,
                    hwnd,
                    ID_BTN_HOME as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_home, WM_SETFONT, ui_font as WPARAM, 1);

                let btn_cern = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("CERN").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    218,
                    10,
                    54,
                    28,
                    hwnd,
                    ID_BTN_CERN as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_cern, WM_SETFONT, ui_font as WPARAM, 1);

                let btn_example = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("HTTPS Example").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    276,
                    10,
                    106,
                    28,
                    hwnd,
                    ID_BTN_EXAMPLE as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_example, WM_SETFONT, ui_font as WPARAM, 1);

                // Adres Çubuğu Metin Kutusu
                let url_edit = CreateWindowExW(
                    0,
                    to_wide("EDIT").as_ptr(),
                    to_wide("http://anka.local/hosgeldiniz").as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
                    388,
                    10,
                    430,
                    28,
                    hwnd,
                    ID_URL_EDIT as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(url_edit, WM_SETFONT, ui_font as WPARAM, 1);

                // Git Butonu
                let btn_go = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("Git ->").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    820,
                    10,
                    70,
                    28,
                    hwnd,
                    ID_BTN_GO as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_go, WM_SETFONT, ui_font as WPARAM, 1);

                if let Ok(mut guard) = DURUM.lock()
                    && let Some(st) = guard.as_mut()
                {
                    st.url_edit = url_edit as usize;
                    st.btn_go = btn_go as usize;
                }
            }
            0
        }

        WM_SIZE => {
            unsafe {
                let mut rc = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                GetClientRect(hwnd, &mut rc);
                let width = rc.right - rc.left;

                if let Ok(guard) = DURUM.lock()
                    && let Some(st) = guard.as_ref()
                {
                    let edit_w = (width - 388 - 88).max(120);
                    MoveWindow(st.url_edit as HWND, 388, 10, edit_w, 28, 1);
                    MoveWindow(st.btn_go as HWND, 388 + edit_w + 8, 10, 72, 28, 1);
                }
                InvalidateRect(hwnd, null(), 1);
            }
            0
        }

        WM_COMMAND => {
            let control_id = wparam & 0xFFFF;
            match control_id {
                ID_BTN_GO => {
                    let adres = {
                        let url_edit = DURUM
                            .lock()
                            .ok()
                            .and_then(|g| g.as_ref().map(|s| s.url_edit as HWND));
                        if let Some(edit_hwnd) = url_edit {
                            let mut buf = [0u16; 1024];
                            let len = unsafe {
                                GetWindowTextW(edit_hwnd, buf.as_mut_ptr(), buf.len() as i32)
                            };
                            String::from_utf16_lossy(&buf[..len.max(0) as usize])
                        } else {
                            String::new()
                        }
                    };
                    if !adres.trim().is_empty() {
                        sayfayi_yukle_ve_yenile(hwnd, &adres, true);
                    }
                }
                ID_BTN_BACK => {
                    if let Ok(mut guard) = DURUM.lock()
                        && let Some(st) = guard.as_mut()
                        && st.state.go_back()
                    {
                        pencereyi_yenile(hwnd, st);
                    }
                }
                ID_BTN_FORWARD => {
                    if let Ok(mut guard) = DURUM.lock()
                        && let Some(st) = guard.as_mut()
                        && st.state.go_forward()
                    {
                        pencereyi_yenile(hwnd, st);
                    }
                }
                ID_BTN_RELOAD => {
                    if let Ok(mut guard) = DURUM.lock()
                        && let Some(st) = guard.as_mut()
                    {
                        st.state.reload();
                        pencereyi_yenile(hwnd, st);
                    }
                }
                ID_BTN_HOME => sayfayi_yukle_ve_yenile(hwnd, "http://anka.local/hosgeldiniz", true),
                ID_BTN_CERN => sayfayi_yukle_ve_yenile(hwnd, "http://info.cern.ch", true),
                ID_BTN_EXAMPLE => sayfayi_yukle_ve_yenile(hwnd, "https://example.com", true),
                _ => {}
            }
            0
        }

        WM_MOUSEMOVE => {
            let mouse_x = (lparam & 0xFFFF) as i16 as i32;
            let mouse_y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let link_var_mi = {
                let guard = DURUM.lock().ok();
                guard.is_some_and(|g| {
                    g.as_ref().is_some_and(|st| {
                        st.state
                            .link_at(mouse_x, mouse_y - TOOLBAR_HEIGHT)
                            .is_some()
                    })
                })
            };

            unsafe {
                let cursor = if link_var_mi { IDC_HAND } else { IDC_ARROW };
                SetCursor(LoadCursorW(null_mut(), cursor));
            }
            0
        }

        WM_LBUTTONDOWN => {
            let click_x = (lparam & 0xFFFF) as i16 as i32;
            let click_y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let hedef_link = {
                let guard = DURUM.lock().ok();
                guard.and_then(|g| {
                    g.as_ref()
                        .and_then(|st| st.state.link_at(click_x, click_y - TOOLBAR_HEIGHT))
                })
            };

            if let Some(yeni_adres) = hedef_link {
                sayfayi_yukle_ve_yenile(hwnd, &yeni_adres, true);
            }
            0
        }

        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) & 0xFFFF) as i16 as i32;
            if let Ok(mut guard) = DURUM.lock()
                && let Some(st) = guard.as_mut()
            {
                st.state.scroll_by(-(delta / 3));
            }
            unsafe {
                InvalidateRect(hwnd, null(), 1);
            }
            0
        }

        WM_PAINT => {
            unsafe {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);

                let mut client = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                GetClientRect(hwnd, &mut client);

                // 1. Üst araç çubuğu arka planı (Açık gri)
                let toolbar_rect = RECT {
                    left: 0,
                    top: 0,
                    right: client.right,
                    bottom: TOOLBAR_HEIGHT,
                };
                let toolbar_brush = CreateSolidBrush(rgb(241, 245, 249));
                FillRect(hdc, &toolbar_rect, toolbar_brush);
                DeleteObject(toolbar_brush as HGDIOBJ);

                // 2. Orta web sayfası tuvali (Bembeyaz)
                let page_rect = RECT {
                    left: 0,
                    top: TOOLBAR_HEIGHT,
                    right: client.right,
                    bottom: client.bottom - STATUS_HEIGHT,
                };
                let white_brush = CreateSolidBrush(rgb(255, 255, 255));
                FillRect(hdc, &page_rect, white_brush);
                DeleteObject(white_brush as HGDIOBJ);

                // 3. Alt durum çubuğu arka planı
                let status_rect = RECT {
                    left: 0,
                    top: client.bottom - STATUS_HEIGHT,
                    right: client.right,
                    bottom: client.bottom,
                };
                let status_brush = CreateSolidBrush(rgb(226, 232, 240));
                FillRect(hdc, &status_rect, status_brush);
                DeleteObject(status_brush as HGDIOBJ);

                SetBkMode(hdc, TRANSPARENT);

                // 4. `anka-paint` çizim listesini gerçek (x, y) koordinatlarına ve RGB renklerine çiz!
                if let Ok(guard) = DURUM.lock()
                    && let Some(st) = guard.as_ref()
                {
                    let face = to_wide("Segoe UI");

                    for komut in &st.state.display_list {
                        let weight = if matches!(komut.tag.as_str(), "h1" | "h2" | "h3") {
                            FW_BOLD
                        } else {
                            FW_NORMAL
                        };
                        let underline = if komut.tag == "a" { 1 } else { 0 };

                        let font = CreateFontW(
                            -(komut.font_size as i32),
                            0,
                            0,
                            0,
                            weight,
                            0,
                            underline,
                            0,
                            DEFAULT_CHARSET,
                            OUT_DEFAULT_PRECIS,
                            CLIP_DEFAULT_PRECIS,
                            CLEARTYPE_QUALITY,
                            DEFAULT_PITCH,
                            face.as_ptr(),
                        );
                        let old_font = SelectObject(hdc, font as HGDIOBJ);

                        SetTextColor(hdc, rgb(komut.color.r, komut.color.g, komut.color.b));

                        let wide_text = to_wide(&komut.text);
                        let draw_x = komut.x as i32 + 8;
                        let draw_y = TOOLBAR_HEIGHT + komut.y as i32 - st.state.scroll_y;

                        if draw_y >= TOOLBAR_HEIGHT && draw_y < client.bottom - STATUS_HEIGHT - 12 {
                            let box_rect = RECT {
                                left: draw_x - 4,
                                top: draw_y - 2,
                                right: (draw_x + komut.width as i32).min(client.right - 16),
                                bottom: draw_y + komut.height as i32,
                            };

                            // Eğer kutunun CSS arka plan rengi (`background-color`) varsa önce kutuyu boya!
                            if let Some(bg) = &komut.bg_color {
                                let bg_brush = CreateSolidBrush(rgb(bg.r, bg.g, bg.b));
                                FillRect(hdc, &box_rect, bg_brush);
                                DeleteObject(bg_brush as HGDIOBJ);
                            }

                            // Eğer kutunun CSS çerçevesi (`border`) varsa etrafına çerçeve çiz!
                            if let Some(border) = &komut.border_color {
                                let border_brush =
                                    CreateSolidBrush(rgb(border.r, border.g, border.b));
                                FrameRect(hdc, &box_rect, border_brush);
                                DeleteObject(border_brush as HGDIOBJ);
                            }

                            if !komut.text.is_empty() {
                                TextOutW(
                                    hdc,
                                    draw_x + 4,
                                    draw_y + 2,
                                    wide_text.as_ptr(),
                                    (wide_text.len() - 1) as i32,
                                );
                            }
                        }

                        SelectObject(hdc, old_font);
                        DeleteObject(font as HGDIOBJ);
                    }

                    // 5. Alt durum çubuğu yazısını çiz
                    let status_font = CreateFontW(
                        -14,
                        0,
                        0,
                        0,
                        FW_NORMAL,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET,
                        OUT_DEFAULT_PRECIS,
                        CLIP_DEFAULT_PRECIS,
                        CLEARTYPE_QUALITY,
                        DEFAULT_PITCH,
                        face.as_ptr(),
                    );
                    let old_font = SelectObject(hdc, status_font as HGDIOBJ);
                    SetTextColor(hdc, rgb(51, 65, 85));
                    let wide_status = to_wide(&st.state.status_message);
                    TextOutW(
                        hdc,
                        12,
                        client.bottom - STATUS_HEIGHT + 6,
                        wide_status.as_ptr(),
                        (wide_status.len() - 1) as i32,
                    );
                    SelectObject(hdc, old_font);
                    DeleteObject(status_font as HGDIOBJ);
                }

                EndPaint(hwnd, &ps);
            }
            0
        }

        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn pencereyi_baslat(baslangic_adresi: &str) {
    let state = BrowserState::new(baslangic_adresi, 880);
    let title_str = state.window_title();

    if let Ok(mut guard) = DURUM.lock() {
        *guard = Some(PencereDurumu {
            url_edit: 0,
            btn_go: 0,
            state,
        });
    }

    unsafe {
        let hinst = GetModuleHandleW(null());
        let class_name = to_wide("AnkaBrowserWindowClass");
        let title = to_wide(&title_str);

        let wc = WNDCLASSW {
            style: 0x0003,
            lpfnWndProc: wnd_proc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: CreateSolidBrush(rgb(255, 255, 255)),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
        };

        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            920,
            640,
            null_mut(),
            null_mut(),
            hinst,
            null_mut(),
        );

        // Eğer kullanıcı komut satırından özel bir adres verdiyse kutuya onu yaz
        if let Ok(guard) = DURUM.lock()
            && let Some(st) = guard.as_ref()
            && st.url_edit != 0
        {
            let wide_url = to_wide(baslangic_adresi);
            SetWindowTextW(st.url_edit as HWND, wide_url.as_ptr());
        }

        ShowWindow(hwnd, 5);
        UpdateWindow(hwnd);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            // Adres çubuğunda Enter tuşuna (`VK_RETURN`) basıldıysa "Git ->" butonunu tetikle!
            if msg.message == WM_KEYDOWN && msg.wParam == VK_RETURN {
                SendMessageW(hwnd, WM_COMMAND, ID_BTN_GO, 0);
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
