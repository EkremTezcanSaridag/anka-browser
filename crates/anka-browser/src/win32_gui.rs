// Anka Browser - Sıfır Bağımlılıklı Yerel Windows Pencere ve GDI Çizim Motoru
// Rust'ın kendi içinde hazır gelen user32 ve gdi32 sistem kütüphanelerini doğrudan kullanır!

#![cfg(windows)]
#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::Mutex;

use crate::browser_state::{BrowserState, ClickAction};

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
const WM_CHAR: u32 = 0x0102;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_MOUSEWHEEL: u32 = 0x020A;

const VK_BACK: usize = 0x08;
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
const ID_BTN_NEW_TAB: usize = 109;
const ID_BTN_CLOSE_TAB: usize = 110;
const ID_BTN_BOOKMARK_TOGGLE: usize = 111;
const ID_BTN_BOOKMARKS_PAGE: usize = 112;
const ID_BTN_SHIELD: usize = 113;
const ID_BTN_TAB_BASE: usize = 200;
const MAX_TAB_SLOTS: usize = 5;

const TOOLBAR_HEIGHT: i32 = 84;
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

#[repr(C)]
struct BITMAPINFOHEADER {
    biSize: u32,
    biWidth: i32,
    biHeight: i32,
    biPlanes: u16,
    biBitCount: u16,
    biCompression: u32,
    biSizeImage: u32,
    biXPelsPerMeter: i32,
    biYPelsPerMeter: i32,
    biClrUsed: u32,
    biClrImportant: u32,
}

#[repr(C)]
struct BITMAPINFO {
    bmiHeader: BITMAPINFOHEADER,
    bmiColors: [u32; 1],
}

const DIB_RGB_COLORS: u32 = 0;
const SRCCOPY: u32 = 0x00CC_0020;

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
    fn SetFocus(hWnd: HWND) -> HWND;
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
    fn StretchDIBits(
        hdc: HDC,
        xDest: i32,
        yDest: i32,
        DestWidth: i32,
        DestHeight: i32,
        xSrc: i32,
        ySrc: i32,
        SrcWidth: i32,
        SrcHeight: i32,
        lpBits: *const c_void,
        lpbmi: *const BITMAPINFO,
        iUsage: u32,
        rop: u32,
    ) -> i32;
}

struct PencereDurumu {
    tab_buttons: [usize; MAX_TAB_SLOTS],
    btn_bookmark_toggle: usize,
    btn_shield: usize,
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
    let wide_url = to_wide(st.state.current_url());
    let wide_title = to_wide(&st.state.window_title());
    let bm_label = if st.state.is_current_bookmarked() {
        "* Kayitli"
    } else {
        "* Yer Imi"
    };
    let wide_bm = to_wide(bm_label);
    let wide_shield = to_wide(&st.state.shield_button_label());

    unsafe {
        SetWindowTextW(url_edit, wide_url.as_ptr());
        SetWindowTextW(hwnd, wide_title.as_ptr());
        if st.btn_bookmark_toggle != 0 {
            SetWindowTextW(st.btn_bookmark_toggle as HWND, wide_bm.as_ptr());
        }
        if st.btn_shield != 0 {
            SetWindowTextW(st.btn_shield as HWND, wide_shield.as_ptr());
        }
        for (idx, &btn_ptr) in st.tab_buttons.iter().enumerate() {
            if btn_ptr != 0 {
                let btn_hwnd = btn_ptr as HWND;
                if idx < st.state.tabs.len() {
                    let lbl = to_wide(&st.state.tab_label(idx));
                    SetWindowTextW(btn_hwnd, lbl.as_ptr());
                    ShowWindow(btn_hwnd, 5); // SW_SHOW
                } else {
                    ShowWindow(btn_hwnd, 0); // SW_HIDE
                }
            }
        }
        InvalidateRect(hwnd, null(), 1);
    }
}

fn sayfayi_istekle_yukle_ve_yenile(
    hwnd: HWND,
    adres: &str,
    post_body: Option<&str>,
    gecmise_ekle: bool,
) {
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
        if let Some(body) = post_body {
            st.state.submit_post(adres, body, gecmise_ekle);
        } else {
            st.state.navigate(adres, gecmise_ekle);
        }
        pencereyi_yenile(hwnd, st);
    }
}

fn sayfayi_yukle_ve_yenile(hwnd: HWND, adres: &str, gecmise_ekle: bool) {
    sayfayi_istekle_yukle_ve_yenile(hwnd, adres, None, gecmise_ekle);
}

fn win32_sayfa_eylemi_uygula(hwnd: HWND, action: ClickAction) {
    match action {
        ClickAction::Navigate(yeni_adres) => {
            sayfayi_istekle_yukle_ve_yenile(hwnd, &yeni_adres, None, true);
        }
        ClickAction::SubmitPost { url, body } => {
            sayfayi_istekle_yukle_ve_yenile(hwnd, &url, Some(&body), true);
        }
        ClickAction::Redraw | ClickAction::None => {}
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

                // 1. SATIR: Çoklu Sekme ve Yer İmleri Çubuğu (Y = 8)
                let mut tab_buttons = [0usize; MAX_TAB_SLOTS];
                for (idx, slot) in tab_buttons.iter_mut().enumerate() {
                    let tab_btn = CreateWindowExW(
                        0,
                        to_wide("BUTTON").as_ptr(),
                        to_wide(&format!("Sekme {}", idx + 1)).as_ptr(),
                        WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                        8 + (idx as i32 * 136),
                        8,
                        132,
                        26,
                        hwnd,
                        (ID_BTN_TAB_BASE + idx) as HMENU,
                        hinst,
                        null_mut(),
                    );
                    SendMessageW(tab_btn, WM_SETFONT, ui_font as WPARAM, 1);
                    *slot = tab_btn as usize;
                }

                let btn_new_tab = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("+ Sekme").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    692,
                    8,
                    72,
                    26,
                    hwnd,
                    ID_BTN_NEW_TAB as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_new_tab, WM_SETFONT, ui_font as WPARAM, 1);

                let btn_close_tab = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("x Kapat").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    768,
                    8,
                    68,
                    26,
                    hwnd,
                    ID_BTN_CLOSE_TAB as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_close_tab, WM_SETFONT, ui_font as WPARAM, 1);

                let btn_bookmarks_page = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("Yer Imlerim").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    840,
                    8,
                    92,
                    26,
                    hwnd,
                    ID_BTN_BOOKMARKS_PAGE as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_bookmarks_page, WM_SETFONT, ui_font as WPARAM, 1);

                // 2. SATIR: Gezinme Butonları (Y = 42): <- Geri, -> İleri, Yenile, Ana Sayfa, CERN, HTTPS Example
                let btn_back = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("<-").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    8,
                    42,
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
                    42,
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
                    42,
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
                    42,
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
                    42,
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
                    42,
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
                    42,
                    380,
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
                    774,
                    42,
                    68,
                    28,
                    hwnd,
                    ID_BTN_GO as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_go, WM_SETFONT, ui_font as WPARAM, 1);

                // Yer İmi Ekle/Çıkar Butonu
                let btn_bookmark_toggle = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("* Yer Imi").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    846,
                    42,
                    86,
                    28,
                    hwnd,
                    ID_BTN_BOOKMARK_TOGGLE as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_bookmark_toggle, WM_SETFONT, ui_font as WPARAM, 1);

                let btn_shield = CreateWindowExW(
                    0,
                    to_wide("BUTTON").as_ptr(),
                    to_wide("Kalkan: Acik").as_ptr(),
                    WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON,
                    936,
                    42,
                    110,
                    28,
                    hwnd,
                    ID_BTN_SHIELD as HMENU,
                    hinst,
                    null_mut(),
                );
                SendMessageW(btn_shield, WM_SETFONT, ui_font as WPARAM, 1);

                if let Ok(mut guard) = DURUM.lock()
                    && let Some(st) = guard.as_mut()
                {
                    st.tab_buttons = tab_buttons;
                    st.btn_bookmark_toggle = btn_bookmark_toggle as usize;
                    st.btn_shield = btn_shield as usize;
                    st.url_edit = url_edit as usize;
                    st.btn_go = btn_go as usize;
                    pencereyi_yenile(hwnd, st);
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
                    let edit_w = (width - 388 - 286).max(120);
                    MoveWindow(st.url_edit as HWND, 388, 42, edit_w, 28, 1);
                    MoveWindow(st.btn_go as HWND, 388 + edit_w + 6, 42, 64, 28, 1);
                    MoveWindow(
                        st.btn_bookmark_toggle as HWND,
                        388 + edit_w + 76,
                        42,
                        84,
                        28,
                        1,
                    );
                    MoveWindow(
                        st.btn_shield as HWND,
                        388 + edit_w + 166,
                        42,
                        110,
                        28,
                        1,
                    );
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
                ID_BTN_BOOKMARKS_PAGE => {
                    sayfayi_yukle_ve_yenile(hwnd, "http://anka.local/yer-imleri", true)
                }
                ID_BTN_NEW_TAB => {
                    if let Ok(mut guard) = DURUM.lock()
                        && let Some(st) = guard.as_mut()
                        && st.state.tabs.len() < MAX_TAB_SLOTS
                    {
                        st.state.new_tab("http://anka.local/hosgeldiniz");
                        pencereyi_yenile(hwnd, st);
                    }
                }
                ID_BTN_CLOSE_TAB => {
                    if let Ok(mut guard) = DURUM.lock()
                        && let Some(st) = guard.as_mut()
                    {
                        st.state.close_active_tab();
                        pencereyi_yenile(hwnd, st);
                    }
                }
                ID_BTN_BOOKMARK_TOGGLE => {
                    if let Ok(mut guard) = DURUM.lock()
                        && let Some(st) = guard.as_mut()
                    {
                        st.state.toggle_bookmark();
                        pencereyi_yenile(hwnd, st);
                    }
                }
                ID_BTN_SHIELD => {
                    if let Ok(mut guard) = DURUM.lock()
                        && let Some(st) = guard.as_mut()
                    {
                        st.state.toggle_shield();
                        pencereyi_yenile(hwnd, st);
                    }
                }
                id if (ID_BTN_TAB_BASE..ID_BTN_TAB_BASE + MAX_TAB_SLOTS).contains(&id) => {
                    let tab_idx = id - ID_BTN_TAB_BASE;
                    if let Ok(mut guard) = DURUM.lock()
                        && let Some(st) = guard.as_mut()
                        && st.state.switch_tab(tab_idx)
                    {
                        pencereyi_yenile(hwnd, st);
                    }
                }
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
                    g.as_ref()
                        .is_some_and(|st| st.state.is_interactive_at(mouse_x, mouse_y - TOOLBAR_HEIGHT))
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
            let action = {
                let mut guard = DURUM.lock().ok();
                if let Some(st) = guard.as_mut().and_then(|g| g.as_mut()) {
                    let act = st.state.click_at(click_x, click_y - TOOLBAR_HEIGHT);
                    if st.state.focused_input().is_some() {
                        unsafe {
                            SetFocus(hwnd);
                        }
                    }
                    if act == ClickAction::Redraw {
                        pencereyi_yenile(hwnd, st);
                    }
                    act
                } else {
                    ClickAction::None
                }
            };

            win32_sayfa_eylemi_uygula(hwnd, action);
            0
        }

        WM_CHAR => {
            let ch_code = wparam as u32;
            let mut submit_action = ClickAction::None;
            if let Ok(mut guard) = DURUM.lock()
                && let Some(st) = guard.as_mut()
                && st.state.focused_input().is_some()
            {
                if ch_code == VK_BACK as u32 {
                    st.state.backspace_input();
                } else if ch_code == VK_RETURN as u32 {
                    submit_action = st.state.submit_focused_form();
                } else if let Some(ch) = char::from_u32(ch_code)
                    && !ch.is_control()
                {
                    st.state.type_char(ch);
                }
                unsafe {
                    InvalidateRect(hwnd, null(), 1);
                }
            }
            win32_sayfa_eylemi_uygula(hwnd, submit_action);
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

                // 2. Orta web sayfası tuvali (Sayfanın kendi `<body>` arka planı veya beyaz)
                let page_rect = RECT {
                    left: 0,
                    top: TOOLBAR_HEIGHT,
                    right: client.right,
                    bottom: client.bottom - STATUS_HEIGHT,
                };
                let page_bg = DURUM
                    .lock()
                    .ok()
                    .and_then(|g| g.as_ref().and_then(|st| st.state.page_bg()))
                    .unwrap_or(anka_paint::RgbColor {
                        r: 255,
                        g: 255,
                        b: 255,
                    });
                let bg_brush = CreateSolidBrush(rgb(page_bg.r, page_bg.g, page_bg.b));
                FillRect(hdc, &page_rect, bg_brush);
                DeleteObject(bg_brush as HGDIOBJ);

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
                    let mono_face = to_wide("Consolas");
                    let scroll_y = st.state.scroll_y();

                    for komut in st.state.display_list() {
                        let weight = if matches!(
                            komut.tag.as_str(),
                            "h1" | "h2" | "h3" | "strong" | "b"
                        ) {
                            FW_BOLD
                        } else {
                            FW_NORMAL
                        };
                        let italic = if matches!(komut.tag.as_str(), "em" | "i") {
                            1
                        } else {
                            0
                        };
                        let underline = if komut.tag == "a"
                            || (komut.href.is_some()
                                && !matches!(
                                    komut.tag.as_str(),
                                    "img" | "button" | "input" | "select"
                                )) {
                            1
                        } else {
                            0
                        };

                        let active_face = if komut.tag == "code" {
                            mono_face.as_ptr()
                        } else {
                            face.as_ptr()
                        };

                        let font = CreateFontW(
                            -(komut.font_size as i32),
                            0,
                            0,
                            0,
                            weight,
                            italic,
                            underline,
                            0,
                            DEFAULT_CHARSET,
                            OUT_DEFAULT_PRECIS,
                            CLIP_DEFAULT_PRECIS,
                            CLEARTYPE_QUALITY,
                            DEFAULT_PITCH,
                            active_face,
                        );
                        let old_font = SelectObject(hdc, font as HGDIOBJ);

                        SetTextColor(hdc, rgb(komut.color.r, komut.color.g, komut.color.b));

                        let wide_text = to_wide(&komut.text);
                        let draw_x = komut.x as i32 + 8;
                        let draw_y = TOOLBAR_HEIGHT + komut.y as i32 - scroll_y;

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

                            // Eğer kutu çözülmüş bir `<img>` görseli içeriyorsa Win32 `StretchDIBits` ile çiz!
                            let mut gorsel_cizildi = false;
                            if let Some(img) = &komut.image
                                && img.width > 0
                                && img.height > 0
                            {
                                let mut bgra = Vec::with_capacity(img.rgba.len());
                                let (px_chunks, _) = img.rgba.as_chunks::<4>();
                                for &[r, g, b, a] in px_chunks {
                                    bgra.push(b);
                                    bgra.push(g);
                                    bgra.push(r);
                                    bgra.push(a);
                                }
                                let bmi = BITMAPINFO {
                                    bmiHeader: BITMAPINFOHEADER {
                                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                                        biWidth: img.width as i32,
                                        // Negatif yükseklik: Piksel verisinin yukarıdan aşağıya sıralı olduğunu bildirir
                                        biHeight: -(img.height as i32),
                                        biPlanes: 1,
                                        biBitCount: 32,
                                        biCompression: 0,
                                        biSizeImage: 0,
                                        biXPelsPerMeter: 0,
                                        biYPelsPerMeter: 0,
                                        biClrUsed: 0,
                                        biClrImportant: 0,
                                    },
                                    bmiColors: [0],
                                };
                                StretchDIBits(
                                    hdc,
                                    box_rect.left,
                                    box_rect.top,
                                    (box_rect.right - box_rect.left).max(1),
                                    (box_rect.bottom - box_rect.top).max(1),
                                    0,
                                    0,
                                    img.width as i32,
                                    img.height as i32,
                                    bgra.as_ptr().cast::<c_void>(),
                                    &bmi,
                                    DIB_RGB_COLORS,
                                    SRCCOPY,
                                );
                                gorsel_cizildi = true;
                            }

                            // Eğer kutunun CSS çerçevesi (`border`) varsa veya odaklı `<input>` ise çiz!
                            let is_focused_input = komut.input_name.is_some()
                                && st.state.focused_input() == komut.input_name.as_deref();
                            if is_focused_input {
                                let border_brush = CreateSolidBrush(rgb(37, 99, 235));
                                FrameRect(hdc, &box_rect, border_brush);
                                DeleteObject(border_brush as HGDIOBJ);
                            } else if let Some(border) = &komut.border_color {
                                let border_brush =
                                    CreateSolidBrush(rgb(border.r, border.g, border.b));
                                FrameRect(hdc, &box_rect, border_brush);
                                DeleteObject(border_brush as HGDIOBJ);
                            }

                            let mut text_color = komut.color;
                            let render_text: String = if let Some(input_name) = &komut.input_name {
                                if let Some(typed) = st.state.input_value(input_name)
                                    && !typed.is_empty()
                                {
                                    text_color = anka_paint::RgbColor {
                                        r: 15,
                                        g: 23,
                                        b: 42,
                                    };
                                    let visible = if komut.is_password {
                                        "\u{2022}".repeat(typed.chars().count())
                                    } else {
                                        typed.to_string()
                                    };
                                    if is_focused_input {
                                        format!("{visible}|")
                                    } else {
                                        visible
                                    }
                                } else if is_focused_input {
                                    "|".to_string()
                                } else {
                                    komut.text.clone()
                                }
                            } else {
                                komut.text.clone()
                            };

                            if !gorsel_cizildi && !render_text.is_empty() {
                                SetTextColor(hdc, rgb(text_color.r, text_color.g, text_color.b));
                                let wide_render = to_wide(&render_text);
                                let pad = komut.padding as i32;
                                let box_w = (box_rect.right - box_rect.left).max(1);
                                let approx_text_w = ((render_text.chars().count() as u32)
                                    * (komut.font_size * 56 / 100))
                                    as i32;
                                let text_x = match komut.text_align.as_str() {
                                    "center" => box_rect.left + ((box_w - approx_text_w) / 2).max(pad),
                                    "right" => box_rect.left + (box_w - approx_text_w - pad).max(pad),
                                    _ => draw_x + 4 + pad,
                                };
                                TextOutW(
                                    hdc,
                                    text_x,
                                    draw_y + 2 + pad,
                                    wide_render.as_ptr(),
                                    (wide_render.len() - 1) as i32,
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
                    let wide_status = to_wide(&st.state.status_message());
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
    let state = BrowserState::new(baslangic_adresi, 940);
    let title_str = state.window_title();

    if let Ok(mut guard) = DURUM.lock() {
        *guard = Some(PencereDurumu {
            tab_buttons: [0usize; MAX_TAB_SLOTS],
            btn_bookmark_toggle: 0,
            btn_shield: 0,
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
            // Adres çubuğunda Enter tuşuna (`VK_RETURN`) basıldıysa (odaklı sayfa `<input>` yoksa) "Git ->" butonunu tetikle!
            let input_focused = DURUM
                .lock()
                .ok()
                .is_some_and(|g| g.as_ref().is_some_and(|s| s.state.focused_input().is_some()));
            if !input_focused && msg.message == WM_KEYDOWN && msg.wParam == VK_RETURN {
                SendMessageW(hwnd, WM_COMMAND, ID_BTN_GO, 0);
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
