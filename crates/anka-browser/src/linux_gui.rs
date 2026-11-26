// Anka Browser - Sıfır Bağımlılıklı Yerel Linux Masaüstü Penceresi (GTK3 + Cairo + Pango + dlopen)
// Derleme sırasında `libgtk-3-dev` / `libcairo2-dev` / `libpango1.0-dev` paketi GEREKTİRMEZ!
// Çalışma zamanında (runtime) sistemde zaten kurulu bulunan `libgtk-3.so.0`, `libgdk-3.so.0`,
// `libgobject-2.0.so.0`, `libcairo.so.2` ve opsiyonel Pango (`libpango-1.0.so.0` / `libpangocairo-1.0.so.0`)
// kütüphanelerini POSIX `dlopen` + `dlsym` ile yükler.
// Böylece hem tüm Linux dağıtımlarında (Wayland & X11) hem de ekransız CI sunucularında kusursuz çalışır.

#![cfg(target_os = "linux")]

use std::ffi::{CStr, CString, c_char, c_double, c_int, c_ulong, c_void};
use std::ptr::{null, null_mut};
use std::sync::{Mutex, OnceLock};

use crate::browser_state::{BrowserState, ClickAction};

// GTK / GDK / Cairo opak C işaretçi tipleri:
type GtkWidget = c_void;
type GtkWindow = c_void;
type GtkContainer = c_void;
type GtkBox = c_void;
type GtkButton = c_void;
type GtkEntry = c_void;
type GtkLabel = c_void;
type GdkEvent = c_void;
type CairoT = c_void;
type CairoSurfaceT = c_void;
type PangoLayout = c_void;
type PangoFontDescription = c_void;
type GCallback = *const c_void;

const RTLD_NOW: c_int = 2;

// GTK Sabitleri:
const GTK_WINDOW_TOPLEVEL: c_int = 0;
const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
const GTK_ORIENTATION_VERTICAL: c_int = 1;

// GDK Fare, Klavye ve Kaydırma Olay Maskeleri:
const GDK_BUTTON_PRESS_MASK: c_int = 1 << 8;
const GDK_KEY_PRESS_MASK: c_int = 1 << 10;
const GDK_SCROLL_MASK: c_int = 1 << 21;
const GDK_SMOOTH_SCROLL_MASK: c_int = 1 << 23;

// GDK Özel Tuş Kodları (`gdkkeysyms.h`):
const GDK_KEY_BACKSPACE: u32 = 0xff08;
const GDK_KEY_RETURN: u32 = 0xff0d;
const GDK_KEY_KP_ENTER: u32 = 0xff8d;

// GDK Kaydırma Yönleri:
const GDK_SCROLL_UP: c_int = 0;
const GDK_SCROLL_DOWN: c_int = 1;

// Cairo Yazı ve Görsel Biçim Sabitleri:
const CAIRO_FONT_SLANT_NORMAL: c_int = 0;
const CAIRO_FONT_SLANT_ITALIC: c_int = 1;
const CAIRO_FONT_WEIGHT_NORMAL: c_int = 0;
const CAIRO_FONT_WEIGHT_BOLD: c_int = 1;
const CAIRO_FORMAT_ARGB32: c_int = 0;

// Sekme çubuğunda aynı anda gösterilecek maksimum hızlı sekme butonu sayısı:
const MAX_TAB_SLOTS: usize = 5;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

/// Çok dilli metin şekillendirme (Arapça, Japonca, Çince, Türkçe ve emoji fallback) ve piksel ölçümü için opsiyonel Pango API:
struct PangoApi {
    pango_cairo_create_layout: unsafe extern "C" fn(*mut CairoT) -> *mut PangoLayout,
    pango_cairo_show_layout: unsafe extern "C" fn(*mut CairoT, *mut PangoLayout),
    pango_font_description_from_string:
        unsafe extern "C" fn(*const c_char) -> *mut PangoFontDescription,
    pango_font_description_set_absolute_size:
        unsafe extern "C" fn(*mut PangoFontDescription, c_double),
    pango_font_description_free: unsafe extern "C" fn(*mut PangoFontDescription),
    pango_layout_set_text: unsafe extern "C" fn(*mut PangoLayout, *const c_char, c_int),
    pango_layout_set_font_description:
        unsafe extern "C" fn(*mut PangoLayout, *const PangoFontDescription),
    pango_layout_get_pixel_size: unsafe extern "C" fn(*mut PangoLayout, *mut c_int, *mut c_int),
    g_object_unref: unsafe extern "C" fn(*mut c_void),
}

/// Çalışma zamanında dinamik olarak yüklenen GTK3 + GDK3 + GObject + Cairo (+ Pango) C fonksiyon tablosu:
struct GtkCairoApi {
    gtk_init_check: unsafe extern "C" fn(*mut c_int, *mut *mut *mut c_char) -> c_int,
    gtk_main: unsafe extern "C" fn(),
    gtk_main_quit: unsafe extern "C" fn(),
    gtk_window_new: unsafe extern "C" fn(c_int) -> *mut GtkWidget,
    gtk_window_set_title: unsafe extern "C" fn(*mut GtkWindow, *const c_char),
    gtk_window_set_default_size: unsafe extern "C" fn(*mut GtkWindow, c_int, c_int),
    gtk_box_new: unsafe extern "C" fn(c_int, c_int) -> *mut GtkWidget,
    gtk_box_pack_start: unsafe extern "C" fn(*mut GtkBox, *mut GtkWidget, c_int, c_int, u32),
    gtk_container_add: unsafe extern "C" fn(*mut GtkContainer, *mut GtkWidget),
    gtk_container_set_border_width: unsafe extern "C" fn(*mut GtkContainer, u32),
    gtk_button_new_with_label: unsafe extern "C" fn(*const c_char) -> *mut GtkWidget,
    gtk_button_set_label: unsafe extern "C" fn(*mut GtkButton, *const c_char),
    gtk_entry_new: unsafe extern "C" fn() -> *mut GtkWidget,
    gtk_entry_set_text: unsafe extern "C" fn(*mut GtkEntry, *const c_char),
    gtk_entry_get_text: unsafe extern "C" fn(*mut GtkEntry) -> *const c_char,
    gtk_label_new: unsafe extern "C" fn(*const c_char) -> *mut GtkWidget,
    gtk_label_set_text: unsafe extern "C" fn(*mut GtkLabel, *const c_char),
    gtk_widget_set_halign: unsafe extern "C" fn(*mut GtkWidget, c_int),
    gtk_drawing_area_new: unsafe extern "C" fn() -> *mut GtkWidget,
    gtk_widget_add_events: unsafe extern "C" fn(*mut GtkWidget, c_int),
    gtk_widget_queue_draw: unsafe extern "C" fn(*mut GtkWidget),
    gtk_widget_get_allocated_width: unsafe extern "C" fn(*mut GtkWidget) -> c_int,
    gtk_widget_get_allocated_height: unsafe extern "C" fn(*mut GtkWidget) -> c_int,
    gtk_widget_show: unsafe extern "C" fn(*mut GtkWidget),
    gtk_widget_hide: unsafe extern "C" fn(*mut GtkWidget),
    gtk_widget_show_all: unsafe extern "C" fn(*mut GtkWidget),
    gtk_widget_set_can_focus: unsafe extern "C" fn(*mut GtkWidget, c_int),
    gtk_widget_grab_focus: unsafe extern "C" fn(*mut GtkWidget),

    gdk_event_get_coords: unsafe extern "C" fn(*const GdkEvent, *mut c_double, *mut c_double) -> c_int,
    gdk_event_get_scroll_direction: unsafe extern "C" fn(*const GdkEvent, *mut c_int) -> c_int,
    gdk_event_get_scroll_deltas:
        unsafe extern "C" fn(*const GdkEvent, *mut c_double, *mut c_double) -> c_int,
    gdk_event_get_keyval: unsafe extern "C" fn(*const GdkEvent, *mut u32) -> c_int,
    gdk_keyval_to_unicode: unsafe extern "C" fn(u32) -> u32,

    g_signal_connect_data: unsafe extern "C" fn(
        *mut c_void,
        *const c_char,
        GCallback,
        *mut c_void,
        *const c_void,
        c_int,
    ) -> c_ulong,

    cairo_set_source_rgb: unsafe extern "C" fn(*mut CairoT, c_double, c_double, c_double),
    cairo_rectangle: unsafe extern "C" fn(*mut CairoT, c_double, c_double, c_double, c_double),
    cairo_fill: unsafe extern "C" fn(*mut CairoT),
    cairo_stroke: unsafe extern "C" fn(*mut CairoT),
    cairo_set_line_width: unsafe extern "C" fn(*mut CairoT, c_double),
    cairo_select_font_face: unsafe extern "C" fn(*mut CairoT, *const c_char, c_int, c_int),
    cairo_set_font_size: unsafe extern "C" fn(*mut CairoT, c_double),
    cairo_move_to: unsafe extern "C" fn(*mut CairoT, c_double, c_double),
    cairo_show_text: unsafe extern "C" fn(*mut CairoT, *const c_char),
    cairo_image_surface_create_for_data:
        unsafe extern "C" fn(*mut u8, c_int, c_int, c_int, c_int) -> *mut CairoSurfaceT,
    cairo_set_source_surface: unsafe extern "C" fn(*mut CairoT, *mut CairoSurfaceT, c_double, c_double),
    cairo_surface_destroy: unsafe extern "C" fn(*mut CairoSurfaceT),
    cairo_save: unsafe extern "C" fn(*mut CairoT),
    cairo_restore: unsafe extern "C" fn(*mut CairoT),
    cairo_translate: unsafe extern "C" fn(*mut CairoT, c_double, c_double),
    cairo_scale: unsafe extern "C" fn(*mut CairoT, c_double, c_double),
    cairo_paint: unsafe extern "C" fn(*mut CairoT),
    pango: Option<PangoApi>,
}

unsafe impl Send for GtkCairoApi {}
unsafe impl Sync for GtkCairoApi {}

static GTK_API: OnceLock<Option<GtkCairoApi>> = OnceLock::new();

unsafe fn open_lib(candidates: &[&CStr]) -> *mut c_void {
    for name in candidates {
        let h = unsafe { dlopen(name.as_ptr(), RTLD_NOW) };
        if !h.is_null() {
            return h;
        }
    }
    null_mut()
}

unsafe fn load_sym<T>(handle: *mut c_void, name: &CStr) -> Option<T> {
    let ptr = unsafe { dlsym(handle, name.as_ptr()) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute_copy(&ptr) })
    }
}

unsafe fn load_pango_api(gobj: *mut c_void) -> Option<PangoApi> {
    let pango = unsafe { open_lib(&[c"libpango-1.0.so.0", c"libpango-1.0.so"]) };
    let pangocairo = unsafe { open_lib(&[c"libpangocairo-1.0.so.0", c"libpangocairo-1.0.so"]) };
    if pango.is_null() || pangocairo.is_null() {
        return None;
    }

    unsafe {
        Some(PangoApi {
            pango_cairo_create_layout: load_sym(pangocairo, c"pango_cairo_create_layout")?,
            pango_cairo_show_layout: load_sym(pangocairo, c"pango_cairo_show_layout")?,
            pango_font_description_from_string: load_sym(
                pango,
                c"pango_font_description_from_string",
            )?,
            pango_font_description_set_absolute_size: load_sym(
                pango,
                c"pango_font_description_set_absolute_size",
            )?,
            pango_font_description_free: load_sym(pango, c"pango_font_description_free")?,
            pango_layout_set_text: load_sym(pango, c"pango_layout_set_text")?,
            pango_layout_set_font_description: load_sym(
                pango,
                c"pango_layout_set_font_description",
            )?,
            pango_layout_get_pixel_size: load_sym(pango, c"pango_layout_get_pixel_size")?,
            g_object_unref: load_sym(gobj, c"g_object_unref")?,
        })
    }
}

fn gtk_api() -> Option<&'static GtkCairoApi> {
    GTK_API
        .get_or_init(|| unsafe {
            let gtk = open_lib(&[c"libgtk-3.so.0", c"libgtk-3.so"]);
            let gdk = open_lib(&[c"libgdk-3.so.0", c"libgdk-3.so"]);
            let gobj = open_lib(&[c"libgobject-2.0.so.0", c"libgobject-2.0.so"]);
            let cairo = open_lib(&[c"libcairo.so.2", c"libcairo.so"]);
            if gtk.is_null() || gdk.is_null() || gobj.is_null() || cairo.is_null() {
                return None;
            }

            Some(GtkCairoApi {
                gtk_init_check: load_sym(gtk, c"gtk_init_check")?,
                gtk_main: load_sym(gtk, c"gtk_main")?,
                gtk_main_quit: load_sym(gtk, c"gtk_main_quit")?,
                gtk_window_new: load_sym(gtk, c"gtk_window_new")?,
                gtk_window_set_title: load_sym(gtk, c"gtk_window_set_title")?,
                gtk_window_set_default_size: load_sym(gtk, c"gtk_window_set_default_size")?,
                gtk_box_new: load_sym(gtk, c"gtk_box_new")?,
                gtk_box_pack_start: load_sym(gtk, c"gtk_box_pack_start")?,
                gtk_container_add: load_sym(gtk, c"gtk_container_add")?,
                gtk_container_set_border_width: load_sym(gtk, c"gtk_container_set_border_width")?,
                gtk_button_new_with_label: load_sym(gtk, c"gtk_button_new_with_label")?,
                gtk_button_set_label: load_sym(gtk, c"gtk_button_set_label")?,
                gtk_entry_new: load_sym(gtk, c"gtk_entry_new")?,
                gtk_entry_set_text: load_sym(gtk, c"gtk_entry_set_text")?,
                gtk_entry_get_text: load_sym(gtk, c"gtk_entry_get_text")?,
                gtk_label_new: load_sym(gtk, c"gtk_label_new")?,
                gtk_label_set_text: load_sym(gtk, c"gtk_label_set_text")?,
                gtk_widget_set_halign: load_sym(gtk, c"gtk_widget_set_halign")?,
                gtk_drawing_area_new: load_sym(gtk, c"gtk_drawing_area_new")?,
                gtk_widget_add_events: load_sym(gtk, c"gtk_widget_add_events")?,
                gtk_widget_queue_draw: load_sym(gtk, c"gtk_widget_queue_draw")?,
                gtk_widget_get_allocated_width: load_sym(gtk, c"gtk_widget_get_allocated_width")?,
                gtk_widget_get_allocated_height: load_sym(
                    gtk,
                    c"gtk_widget_get_allocated_height",
                )?,
                gtk_widget_show: load_sym(gtk, c"gtk_widget_show")?,
                gtk_widget_hide: load_sym(gtk, c"gtk_widget_hide")?,
                gtk_widget_show_all: load_sym(gtk, c"gtk_widget_show_all")?,
                gtk_widget_set_can_focus: load_sym(gtk, c"gtk_widget_set_can_focus")?,
                gtk_widget_grab_focus: load_sym(gtk, c"gtk_widget_grab_focus")?,

                gdk_event_get_coords: load_sym(gdk, c"gdk_event_get_coords")?,
                gdk_event_get_scroll_direction: load_sym(gdk, c"gdk_event_get_scroll_direction")?,
                gdk_event_get_scroll_deltas: load_sym(gdk, c"gdk_event_get_scroll_deltas")?,
                gdk_event_get_keyval: load_sym(gdk, c"gdk_event_get_keyval")?,
                gdk_keyval_to_unicode: load_sym(gdk, c"gdk_keyval_to_unicode")?,

                g_signal_connect_data: load_sym(gobj, c"g_signal_connect_data")?,

                cairo_set_source_rgb: load_sym(cairo, c"cairo_set_source_rgb")?,
                cairo_rectangle: load_sym(cairo, c"cairo_rectangle")?,
                cairo_fill: load_sym(cairo, c"cairo_fill")?,
                cairo_stroke: load_sym(cairo, c"cairo_stroke")?,
                cairo_set_line_width: load_sym(cairo, c"cairo_set_line_width")?,
                cairo_select_font_face: load_sym(cairo, c"cairo_select_font_face")?,
                cairo_set_font_size: load_sym(cairo, c"cairo_set_font_size")?,
                cairo_move_to: load_sym(cairo, c"cairo_move_to")?,
                cairo_show_text: load_sym(cairo, c"cairo_show_text")?,
                cairo_image_surface_create_for_data: load_sym(
                    cairo,
                    c"cairo_image_surface_create_for_data",
                )?,
                cairo_set_source_surface: load_sym(cairo, c"cairo_set_source_surface")?,
                cairo_surface_destroy: load_sym(cairo, c"cairo_surface_destroy")?,
                cairo_save: load_sym(cairo, c"cairo_save")?,
                cairo_restore: load_sym(cairo, c"cairo_restore")?,
                cairo_translate: load_sym(cairo, c"cairo_translate")?,
                cairo_scale: load_sym(cairo, c"cairo_scale")?,
                cairo_paint: load_sym(cairo, c"cairo_paint")?,
                pango: load_pango_api(gobj),
            })
        })
        .as_ref()
}

/// Linux penceresinin arayüz bileşenlerini ve ortak `BrowserState` durumunu tutan yapı:
struct LinuxGuiApp {
    window: usize,
    tab_buttons: [usize; MAX_TAB_SLOTS],
    btn_bookmark_toggle: usize,
    btn_shield: usize,
    url_entry: usize,
    canvas: usize,
    status_label: usize,
    state: BrowserState,
}

static LINUX_APP: Mutex<Option<LinuxGuiApp>> = Mutex::new(None);

fn to_cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new("").unwrap())
}

unsafe fn connect_signal_with_data(
    api: &GtkCairoApi,
    instance: *mut GtkWidget,
    signal_name: &str,
    callback: GCallback,
    user_data: usize,
) {
    let c_sig = to_cstr(signal_name);
    unsafe {
        (api.g_signal_connect_data)(
            instance,
            c_sig.as_ptr(),
            callback,
            user_data as *mut c_void,
            null(),
            0,
        );
    }
}

unsafe fn connect_signal(
    api: &GtkCairoApi,
    instance: *mut GtkWidget,
    signal_name: &str,
    callback: GCallback,
) {
    unsafe {
        connect_signal_with_data(api, instance, signal_name, callback, 0);
    }
}

fn arayuzu_guncelle(api: &GtkCairoApi, app: &LinuxGuiApp) {
    let title_c = to_cstr(&app.state.window_title());
    let url_c = to_cstr(app.state.current_url());
    let status_c = to_cstr(&app.state.status_message());
    let bm_label = if app.state.is_current_bookmarked() {
        "★ Kayitli"
    } else {
        "☆ Yer Imi"
    };
    let bm_c = to_cstr(bm_label);
    let shield_c = to_cstr(&app.state.shield_button_label());

    unsafe {
        (api.gtk_window_set_title)(app.window as *mut GtkWindow, title_c.as_ptr());
        (api.gtk_entry_set_text)(app.url_entry as *mut GtkEntry, url_c.as_ptr());
        (api.gtk_label_set_text)(app.status_label as *mut GtkLabel, status_c.as_ptr());
        (api.gtk_button_set_label)(app.btn_bookmark_toggle as *mut GtkButton, bm_c.as_ptr());
        (api.gtk_button_set_label)(app.btn_shield as *mut GtkButton, shield_c.as_ptr());

        for (idx, &btn_ptr) in app.tab_buttons.iter().enumerate() {
            let btn = btn_ptr as *mut GtkWidget;
            if idx < app.state.tabs.len() {
                let lbl_c = to_cstr(&app.state.tab_label(idx));
                (api.gtk_button_set_label)(btn as *mut GtkButton, lbl_c.as_ptr());
                (api.gtk_widget_show)(btn);
            } else {
                (api.gtk_widget_hide)(btn);
            }
        }

        (api.gtk_widget_queue_draw)(app.canvas as *mut GtkWidget);
    }
}

fn adres_yukle(hedef: &str, gecmise_ekle: bool) {
    let Some(api) = gtk_api() else { return };
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        let alloc_w = unsafe { (api.gtk_widget_get_allocated_width)(app.canvas as *mut GtkWidget) };
        if alloc_w > 300 {
            app.state.viewport_width = alloc_w as u32;
        }
        app.state.navigate(hedef, gecmise_ekle);
        arayuzu_guncelle(api, app);
    }
}

// ============================================================================
// GTK OLAY GERİ ÇAĞIRIMLARI (EVENT CALLBACKS)
// ============================================================================

unsafe extern "C" fn on_window_destroy(_widget: *mut GtkWidget, _data: *mut c_void) {
    if let Some(api) = gtk_api() {
        unsafe {
            (api.gtk_main_quit)();
        }
    }
}

unsafe extern "C" fn on_tab_clicked(_widget: *mut GtkWidget, data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    let tab_index = data as usize;
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
        && app.state.switch_tab(tab_index)
    {
        arayuzu_guncelle(api, app);
    }
}

unsafe extern "C" fn on_new_tab_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
        && app.state.tabs.len() < MAX_TAB_SLOTS
    {
        app.state.new_tab("http://anka.local/hosgeldiniz");
        arayuzu_guncelle(api, app);
    }
}

unsafe extern "C" fn on_close_tab_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        app.state.close_active_tab();
        arayuzu_guncelle(api, app);
    }
}

unsafe extern "C" fn on_bookmark_toggle_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        app.state.toggle_bookmark();
        arayuzu_guncelle(api, app);
    }
}

unsafe extern "C" fn on_bookmarks_page_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    adres_yukle("http://anka.local/yer-imleri", true);
}

unsafe extern "C" fn on_shield_toggle_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        app.state.toggle_shield();
        arayuzu_guncelle(api, app);
    }
}

unsafe extern "C" fn on_navigate_triggered(_widget: *mut GtkWidget, _data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    let girilen_adres = if let Ok(guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_ref()
    {
        unsafe {
            let ptr = (api.gtk_entry_get_text)(app.url_entry as *mut GtkEntry);
            if ptr.is_null() {
                String::new()
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        }
    } else {
        String::new()
    };

    if !girilen_adres.trim().is_empty() {
        adres_yukle(&girilen_adres, true);
    }
}

unsafe extern "C" fn on_back_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
        && app.state.go_back()
    {
        arayuzu_guncelle(api, app);
    }
}

unsafe extern "C" fn on_forward_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
        && app.state.go_forward()
    {
        arayuzu_guncelle(api, app);
    }
}

unsafe extern "C" fn on_reload_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    let Some(api) = gtk_api() else { return };
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        app.state.reload();
        arayuzu_guncelle(api, app);
    }
}

unsafe extern "C" fn on_home_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    adres_yukle("http://anka.local/hosgeldiniz", true);
}

unsafe extern "C" fn on_cern_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    adres_yukle("http://info.cern.ch", true);
}

unsafe extern "C" fn on_example_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    adres_yukle("https://example.com", true);
}

unsafe extern "C" fn on_canvas_button_press(
    widget: *mut GtkWidget,
    event: *const GdkEvent,
    _data: *mut c_void,
) -> c_int {
    let Some(api) = gtk_api() else { return 0 };
    let mut x: c_double = 0.0;
    let mut y: c_double = 0.0;
    let ok = unsafe { (api.gdk_event_get_coords)(event, &mut x, &mut y) };
    if ok == 0 {
        return 0;
    }

    let action = if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        let act = app.state.click_at(x as i32, y as i32);
        if app.state.focused_input().is_some() {
            unsafe {
                (api.gtk_widget_grab_focus)(widget);
            }
        }
        if act == ClickAction::Redraw {
            arayuzu_guncelle(api, app);
        }
        act
    } else {
        ClickAction::None
    };

    if let ClickAction::Navigate(yeni_url) = action {
        adres_yukle(&yeni_url, true);
    }

    0
}

/// Tuval üzerindeki odaklı bir HTML `<input>` kutusuna klavyeden harf yazıldığında,
/// `Backspace` ile silindiğinde veya `Enter` ile form gönderildiğinde çalışır!
unsafe extern "C" fn on_canvas_key_press(
    widget: *mut GtkWidget,
    event: *const GdkEvent,
    _data: *mut c_void,
) -> c_int {
    let Some(api) = gtk_api() else { return 0 };
    let mut keyval: u32 = 0;
    if unsafe { (api.gdk_event_get_keyval)(event, &mut keyval) } == 0 {
        return 0;
    }

    let mut submit_url: Option<String> = None;
    let mut handled = false;

    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
        && app.state.focused_input().is_some()
    {
        if keyval == GDK_KEY_BACKSPACE {
            handled = app.state.backspace_input();
        } else if keyval == GDK_KEY_RETURN || keyval == GDK_KEY_KP_ENTER {
            submit_url = app.state.submit_focused_form();
            handled = true;
        } else {
            let uni = unsafe { (api.gdk_keyval_to_unicode)(keyval) };
            if let Some(ch) = char::from_u32(uni)
                && !ch.is_control()
            {
                handled = app.state.type_char(ch);
            }
        }
        if handled {
            unsafe {
                (api.gtk_widget_queue_draw)(widget);
            }
        }
    }

    if let Some(url) = submit_url {
        adres_yukle(&url, true);
    }

    if handled { 1 } else { 0 }
}

unsafe extern "C" fn on_canvas_scroll(
    widget: *mut GtkWidget,
    event: *const GdkEvent,
    _data: *mut c_void,
) -> c_int {
    let Some(api) = gtk_api() else { return 0 };
    let mut dir: c_int = -1;
    let mut delta_x: c_double = 0.0;
    let mut delta_y: c_double = 0.0;

    let scroll_step = unsafe {
        if (api.gdk_event_get_scroll_direction)(event, &mut dir) != 0 {
            match dir {
                GDK_SCROLL_UP => -40,
                GDK_SCROLL_DOWN => 40,
                _ => 0,
            }
        } else if (api.gdk_event_get_scroll_deltas)(event, &mut delta_x, &mut delta_y) != 0 {
            (delta_y * 36.0) as i32
        } else {
            0
        }
    };

    if scroll_step != 0
        && let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        app.state.scroll_by(scroll_step);
        unsafe {
            (api.gtk_widget_queue_draw)(widget);
        }
    }

    0
}

unsafe extern "C" fn on_canvas_draw(
    widget: *mut GtkWidget,
    cr: *mut CairoT,
    _data: *mut c_void,
) -> c_int {
    let Some(api) = gtk_api() else { return 0 };
    unsafe {
        let width = (api.gtk_widget_get_allocated_width)(widget) as c_double;
        let height = (api.gtk_widget_get_allocated_height)(widget) as c_double;

        let font_family = to_cstr("Sans");

        if let Ok(guard) = LINUX_APP.lock()
            && let Some(app) = guard.as_ref()
        {
            // 1. Sayfanın kendi <body> arka planını (yoksa beyazı) boya
            let page_bg = app.state.page_bg().unwrap_or(anka_paint::RgbColor {
                r: 255,
                g: 255,
                b: 255,
            });
            (api.cairo_set_source_rgb)(
                cr,
                page_bg.r as c_double / 255.0,
                page_bg.g as c_double / 255.0,
                page_bg.b as c_double / 255.0,
            );
            (api.cairo_rectangle)(cr, 0.0, 0.0, width, height);
            (api.cairo_fill)(cr);

            let scroll_y = app.state.scroll_y();
            for komut in app.state.display_list() {
                let draw_x = (komut.x as c_double) + 8.0;
                let draw_y = (komut.y as i32 - scroll_y) as c_double;
                let box_w = (komut.width as c_double).min((width - 24.0).max(100.0));
                let box_h = komut.height as c_double;

                if draw_y + box_h < 0.0 || draw_y > height {
                    continue;
                }

                // 2. Kutu arka plan rengi (`background-color`)
                if let Some(bg) = &komut.bg_color {
                    (api.cairo_set_source_rgb)(
                        cr,
                        bg.r as c_double / 255.0,
                        bg.g as c_double / 255.0,
                        bg.b as c_double / 255.0,
                    );
                    (api.cairo_rectangle)(cr, draw_x - 4.0, draw_y - 2.0, box_w, box_h);
                    (api.cairo_fill)(cr);
                }

                // 2B. Gerçek `<img>` piksel çizimi
                let mut gorsel_cizildi = false;
                if let Some(img) = &komut.image
                    && img.width > 0
                    && img.height > 0
                {
                    let mut bgra = Vec::with_capacity(img.rgba.len());
                    let (px_chunks, _) = img.rgba.as_chunks::<4>();
                    for &[r_raw, g_raw, b_raw, a_raw] in px_chunks {
                        let a = a_raw as u16;
                        let r = ((r_raw as u16 * a) / 255) as u8;
                        let g = ((g_raw as u16 * a) / 255) as u8;
                        let b = ((b_raw as u16 * a) / 255) as u8;
                        bgra.push(b);
                        bgra.push(g);
                        bgra.push(r);
                        bgra.push(a_raw);
                    }
                    let stride = (img.width * 4) as c_int;
                    let surface = (api.cairo_image_surface_create_for_data)(
                        bgra.as_mut_ptr(),
                        CAIRO_FORMAT_ARGB32,
                        img.width as c_int,
                        img.height as c_int,
                        stride,
                    );
                    if !surface.is_null() {
                        (api.cairo_save)(cr);
                        (api.cairo_translate)(cr, draw_x - 4.0, draw_y - 2.0);
                        (api.cairo_scale)(
                            cr,
                            box_w / (img.width as c_double),
                            box_h / (img.height as c_double),
                        );
                        (api.cairo_set_source_surface)(cr, surface, 0.0, 0.0);
                        (api.cairo_paint)(cr);
                        (api.cairo_restore)(cr);
                        (api.cairo_surface_destroy)(surface);
                        gorsel_cizildi = true;
                    }
                }

                // 3. Kutu çerçevesi (`border`) — Odaklı `<input>` ise canlı mavi çerçeve çiz!
                let is_focused_input = komut.input_name.is_some()
                    && app.state.focused_input() == komut.input_name.as_deref();
                if is_focused_input {
                    (api.cairo_set_source_rgb)(cr, 37.0 / 255.0, 99.0 / 255.0, 235.0 / 255.0);
                    (api.cairo_set_line_width)(cr, 2.0);
                    (api.cairo_rectangle)(cr, draw_x - 4.0, draw_y - 2.0, box_w, box_h);
                    (api.cairo_stroke)(cr);
                } else if let Some(border) = &komut.border_color {
                    (api.cairo_set_source_rgb)(
                        cr,
                        border.r as c_double / 255.0,
                        border.g as c_double / 255.0,
                        border.b as c_double / 255.0,
                    );
                    (api.cairo_set_line_width)(cr, 1.0);
                    (api.cairo_rectangle)(cr, draw_x - 4.0, draw_y - 2.0, box_w, box_h);
                    (api.cairo_stroke)(cr);
                }

                // 4. Metin ve link alt çizgisi (`<input>` ise kullanıcının yazdığı canlı değeri göster!)
                let mut live_text_color = komut.color;
                let render_text: String = if let Some(input_name) = &komut.input_name {
                    if let Some(typed) = app.state.input_value(input_name)
                        && !typed.is_empty()
                    {
                        live_text_color = anka_paint::RgbColor {
                            r: 15,
                            g: 23,
                            b: 42,
                        };
                        if is_focused_input {
                            format!("{typed}|")
                        } else {
                            typed.to_string()
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
                    let is_bold = matches!(
                        komut.tag.as_str(),
                        "h1" | "h2" | "h3" | "strong" | "b"
                    );
                    let is_italic = matches!(komut.tag.as_str(), "em" | "i");
                    let is_code = komut.tag == "code";
                    let is_link_text = komut.tag == "a"
                        || (komut.href.is_some()
                            && !matches!(
                                komut.tag.as_str(),
                                "img" | "button" | "input" | "select"
                            ));

                    (api.cairo_set_source_rgb)(
                        cr,
                        live_text_color.r as c_double / 255.0,
                        live_text_color.g as c_double / 255.0,
                        live_text_color.b as c_double / 255.0,
                    );

                    let pad = komut.padding as c_double;
                    let c_text = to_cstr(&render_text);

                    if let Some(pango) = &api.pango {
                        let font_desc_c = match (is_code, is_bold, is_italic) {
                            (true, true, _) => c"Monospace Bold",
                            (true, false, _) => c"Monospace",
                            (false, true, true) => c"Sans Bold Italic",
                            (false, true, false) => c"Sans Bold",
                            (false, false, true) => c"Sans Italic",
                            (false, false, false) => c"Sans",
                        };
                        let layout = (pango.pango_cairo_create_layout)(cr);
                        let desc = (pango.pango_font_description_from_string)(font_desc_c.as_ptr());
                        if !layout.is_null() {
                            if !desc.is_null() {
                                (pango.pango_font_description_set_absolute_size)(
                                    desc,
                                    (komut.font_size as c_double) * 1024.0,
                                );
                                (pango.pango_layout_set_font_description)(layout, desc);
                            }
                            (pango.pango_layout_set_text)(layout, c_text.as_ptr(), -1);

                            let mut tw: c_int = 0;
                            let mut th: c_int = 0;
                            (pango.pango_layout_get_pixel_size)(layout, &mut tw, &mut th);

                            let real_w = (tw as c_double)
                                .min((box_w - pad * 2.0).max(0.0))
                                .max(0.0);
                            let text_x = match komut.text_align.as_str() {
                                "center" => (draw_x - 4.0) + ((box_w - real_w) / 2.0).max(pad),
                                "right" => (draw_x - 4.0) + (box_w - real_w - pad).max(pad),
                                _ => draw_x + 4.0 + pad,
                            };

                            let top_y = draw_y + pad + 2.0;
                            (api.cairo_move_to)(cr, text_x, top_y);
                            (pango.pango_cairo_show_layout)(cr, layout);

                            if is_link_text {
                                let underline_y = top_y
                                    + (th as c_double).max(komut.font_size as c_double)
                                    - 1.0;
                                (api.cairo_set_line_width)(cr, 1.0);
                                (api.cairo_rectangle)(cr, text_x, underline_y, real_w, 1.0);
                                (api.cairo_fill)(cr);
                            }
                        }
                        if !desc.is_null() {
                            (pango.pango_font_description_free)(desc);
                        }
                        if !layout.is_null() {
                            (pango.g_object_unref)(layout.cast());
                        }
                    } else {
                        let weight = if is_bold {
                            CAIRO_FONT_WEIGHT_BOLD
                        } else {
                            CAIRO_FONT_WEIGHT_NORMAL
                        };
                        let slant = if is_italic {
                            CAIRO_FONT_SLANT_ITALIC
                        } else {
                            CAIRO_FONT_SLANT_NORMAL
                        };

                        (api.cairo_select_font_face)(
                            cr,
                            font_family.as_ptr(),
                            slant,
                            weight,
                        );
                        (api.cairo_set_font_size)(cr, komut.font_size as c_double);

                        let approx_text_w = ((render_text.chars().count() as c_double)
                            * (komut.font_size as c_double * 0.56))
                            .min(box_w - pad * 2.0)
                            .max(0.0);

                        let text_x = match komut.text_align.as_str() {
                            "center" => (draw_x - 4.0) + ((box_w - approx_text_w) / 2.0).max(pad),
                            "right" => (draw_x - 4.0) + (box_w - approx_text_w - pad).max(pad),
                            _ => draw_x + 4.0 + pad,
                        };

                        let baseline_y = draw_y + pad + (komut.font_size as c_double);
                        (api.cairo_move_to)(cr, text_x, baseline_y);
                        (api.cairo_show_text)(cr, c_text.as_ptr());

                        if is_link_text {
                            (api.cairo_set_line_width)(cr, 1.0);
                            (api.cairo_rectangle)(cr, text_x, baseline_y + 2.0, approx_text_w, 1.0);
                            (api.cairo_fill)(cr);
                        }
                    }
                }
            }
        } else {
            (api.cairo_set_source_rgb)(cr, 1.0, 1.0, 1.0);
            (api.cairo_rectangle)(cr, 0.0, 0.0, width, height);
            (api.cairo_fill)(cr);
        }
    }
    0
}

/// Linux masaüstü ortamında (Wayland veya X11) GTK3 penceresini `dlopen` üzerinden başlatır.
pub fn pencereyi_baslat(baslangic_adresi: &str) -> bool {
    let Some(api) = gtk_api() else {
        return false;
    };

    unsafe {
        let mut argc: c_int = 0;
        let mut argv: *mut *mut c_char = null_mut();
        if (api.gtk_init_check)(&mut argc, &mut argv) == 0 {
            return false;
        }

        let state = BrowserState::new(baslangic_adresi, 960);

        let window = (api.gtk_window_new)(GTK_WINDOW_TOPLEVEL);
        let title_c = to_cstr(&state.window_title());
        (api.gtk_window_set_title)(window as *mut GtkWindow, title_c.as_ptr());
        (api.gtk_window_set_default_size)(window as *mut GtkWindow, 1020, 720);
        connect_signal(api, window, "destroy", on_window_destroy as GCallback);

        let vbox = (api.gtk_box_new)(GTK_ORIENTATION_VERTICAL, 0);
        (api.gtk_container_add)(window as *mut GtkContainer, vbox);

        // ÜST SEKME ÇUBUĞU
        let tab_bar = (api.gtk_box_new)(GTK_ORIENTATION_HORIZONTAL, 4);
        (api.gtk_container_set_border_width)(tab_bar as *mut GtkContainer, 4);
        (api.gtk_box_pack_start)(vbox as *mut GtkBox, tab_bar, 0, 1, 0);

        let mut tab_buttons = [0usize; MAX_TAB_SLOTS];
        for (idx, slot) in tab_buttons.iter_mut().enumerate() {
            let lbl = to_cstr(&state.tab_label(idx));
            let btn = (api.gtk_button_new_with_label)(lbl.as_ptr());
            (api.gtk_box_pack_start)(tab_bar as *mut GtkBox, btn, 0, 0, 0);
            connect_signal_with_data(api, btn, "clicked", on_tab_clicked as GCallback, idx);
            *slot = btn as usize;
        }

        let btn_new_tab = (api.gtk_button_new_with_label)(to_cstr("+ Sekme").as_ptr());
        let btn_close_tab = (api.gtk_button_new_with_label)(to_cstr("x Kapat").as_ptr());
        let btn_bookmarks_page = (api.gtk_button_new_with_label)(to_cstr("Yer Imlerim").as_ptr());

        (api.gtk_box_pack_start)(tab_bar as *mut GtkBox, btn_new_tab, 0, 0, 0);
        (api.gtk_box_pack_start)(tab_bar as *mut GtkBox, btn_close_tab, 0, 0, 0);
        (api.gtk_box_pack_start)(tab_bar as *mut GtkBox, btn_bookmarks_page, 0, 0, 4);

        connect_signal(api, btn_new_tab, "clicked", on_new_tab_clicked as GCallback);
        connect_signal(
            api,
            btn_close_tab,
            "clicked",
            on_close_tab_clicked as GCallback,
        );
        connect_signal(
            api,
            btn_bookmarks_page,
            "clicked",
            on_bookmarks_page_clicked as GCallback,
        );

        // GEZİNME VE ADRES ÇUBUĞU
        let toolbar = (api.gtk_box_new)(GTK_ORIENTATION_HORIZONTAL, 6);
        (api.gtk_container_set_border_width)(toolbar as *mut GtkContainer, 4);
        (api.gtk_box_pack_start)(vbox as *mut GtkBox, toolbar, 0, 1, 0);

        let btn_back = (api.gtk_button_new_with_label)(to_cstr("<-").as_ptr());
        let btn_forward = (api.gtk_button_new_with_label)(to_cstr("->").as_ptr());
        let btn_reload = (api.gtk_button_new_with_label)(to_cstr("Yenile").as_ptr());
        let btn_home = (api.gtk_button_new_with_label)(to_cstr("Ana Sayfa").as_ptr());
        let btn_cern = (api.gtk_button_new_with_label)(to_cstr("CERN").as_ptr());
        let btn_example = (api.gtk_button_new_with_label)(to_cstr("HTTPS Example").as_ptr());
        let url_entry = (api.gtk_entry_new)();
        let btn_go = (api.gtk_button_new_with_label)(to_cstr("Git ->").as_ptr());
        let btn_bookmark_toggle = (api.gtk_button_new_with_label)(to_cstr("☆ Yer Imi").as_ptr());
        let btn_shield =
            (api.gtk_button_new_with_label)(to_cstr(&state.shield_button_label()).as_ptr());

        (api.gtk_entry_set_text)(
            url_entry as *mut GtkEntry,
            to_cstr(state.current_url()).as_ptr(),
        );

        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_back, 0, 0, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_forward, 0, 0, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_reload, 0, 0, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_home, 0, 0, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_cern, 0, 0, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_example, 0, 0, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, url_entry, 1, 1, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_go, 0, 0, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_bookmark_toggle, 0, 0, 0);
        (api.gtk_box_pack_start)(toolbar as *mut GtkBox, btn_shield, 0, 0, 0);

        connect_signal(api, btn_back, "clicked", on_back_clicked as GCallback);
        connect_signal(api, btn_forward, "clicked", on_forward_clicked as GCallback);
        connect_signal(api, btn_reload, "clicked", on_reload_clicked as GCallback);
        connect_signal(api, btn_home, "clicked", on_home_clicked as GCallback);
        connect_signal(api, btn_cern, "clicked", on_cern_clicked as GCallback);
        connect_signal(api, btn_example, "clicked", on_example_clicked as GCallback);
        connect_signal(api, btn_go, "clicked", on_navigate_triggered as GCallback);
        connect_signal(
            api,
            url_entry,
            "activate",
            on_navigate_triggered as GCallback,
        );
        connect_signal(
            api,
            btn_bookmark_toggle,
            "clicked",
            on_bookmark_toggle_clicked as GCallback,
        );
        connect_signal(
            api,
            btn_shield,
            "clicked",
            on_shield_toggle_clicked as GCallback,
        );

        // ORTA WEB TUVALİ
        let canvas = (api.gtk_drawing_area_new)();
        (api.gtk_widget_set_can_focus)(canvas, 1);
        (api.gtk_widget_add_events)(
            canvas,
            GDK_BUTTON_PRESS_MASK | GDK_KEY_PRESS_MASK | GDK_SCROLL_MASK | GDK_SMOOTH_SCROLL_MASK,
        );
        (api.gtk_box_pack_start)(vbox as *mut GtkBox, canvas, 1, 1, 0);

        connect_signal(api, canvas, "draw", on_canvas_draw as GCallback);
        connect_signal(
            api,
            canvas,
            "button-press-event",
            on_canvas_button_press as GCallback,
        );
        connect_signal(
            api,
            canvas,
            "key-press-event",
            on_canvas_key_press as GCallback,
        );
        connect_signal(api, canvas, "scroll-event", on_canvas_scroll as GCallback);

        // ALT DURUM ÇUBUĞU
        let status_box = (api.gtk_box_new)(GTK_ORIENTATION_HORIZONTAL, 4);
        (api.gtk_container_set_border_width)(status_box as *mut GtkContainer, 6);
        let status_label = (api.gtk_label_new)(to_cstr(&state.status_message()).as_ptr());
        (api.gtk_widget_set_halign)(status_label, 1);
        (api.gtk_box_pack_start)(status_box as *mut GtkBox, status_label, 1, 1, 4);
        (api.gtk_box_pack_start)(vbox as *mut GtkBox, status_box, 0, 1, 0);

        let app = LinuxGuiApp {
            window: window as usize,
            tab_buttons,
            btn_bookmark_toggle: btn_bookmark_toggle as usize,
            btn_shield: btn_shield as usize,
            url_entry: url_entry as usize,
            canvas: canvas as usize,
            status_label: status_label as usize,
            state,
        };

        (api.gtk_widget_show_all)(window);
        arayuzu_guncelle(api, &app);

        if let Ok(mut guard) = LINUX_APP.lock() {
            *guard = Some(app);
        }

        (api.gtk_main)();
        true
    }
}
