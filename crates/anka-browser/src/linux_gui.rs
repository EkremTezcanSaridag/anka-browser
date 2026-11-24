// Anka Browser - Sıfır Bağımlılıklı Yerel Linux Masaüstü Penceresi (GTK3 + Cairo 2D)
// Tüm Linux dağıtımlarında (Milis, Ubuntu, Debian, Fedora, Arch, Pardus) hem Wayland hem X11
// üzerinde çalışır. Hiçbir harici crate kullanmadan doğrudan sistemdeki `libgtk-3.so`,
// `libgdk-3.so`, `libgobject-2.0.so` ve `libcairo.so` kütüphanelerini kullanır.

#![cfg(target_os = "linux")]

use std::ffi::{CStr, CString, c_char, c_double, c_int, c_ulong, c_void};
use std::ptr::{null, null_mut};
use std::sync::Mutex;

use crate::browser_state::BrowserState;

// GTK / GDK / Cairo opak C işaretçi tipleri:
type GtkWidget = c_void;
type GtkWindow = c_void;
type GtkContainer = c_void;
type GtkBox = c_void;
type GtkEntry = c_void;
type GtkLabel = c_void;
type GdkEvent = c_void;
type CairoT = c_void;
type GCallback = *const c_void;

// GTK Sabitleri:
const GTK_WINDOW_TOPLEVEL: c_int = 0;
const GTK_ORIENTATION_HORIZONTAL: c_int = 0;
const GTK_ORIENTATION_VERTICAL: c_int = 1;

// GDK Fare ve Kaydırma Olay Maskeleri (Event Mask):
const GDK_BUTTON_PRESS_MASK: c_int = 1 << 8;
const GDK_SCROLL_MASK: c_int = 1 << 21;
const GDK_SMOOTH_SCROLL_MASK: c_int = 1 << 23;

// GDK Kaydırma Yönleri (`GdkScrollDirection`):
const GDK_SCROLL_UP: c_int = 0;
const GDK_SCROLL_DOWN: c_int = 1;

// Cairo Yazı Kalınlığı (`cairo_font_weight_t`):
const CAIRO_FONT_SLANT_NORMAL: c_int = 0;
const CAIRO_FONT_WEIGHT_NORMAL: c_int = 0;
const CAIRO_FONT_WEIGHT_BOLD: c_int = 1;

#[link(name = "gtk-3")]
unsafe extern "C" {
    fn gtk_init_check(argc: *mut c_int, argv: *mut *mut *mut c_char) -> c_int;
    fn gtk_main();
    fn gtk_main_quit();

    fn gtk_window_new(window_type: c_int) -> *mut GtkWidget;
    fn gtk_window_set_title(window: *mut GtkWindow, title: *const c_char);
    fn gtk_window_set_default_size(window: *mut GtkWindow, width: c_int, height: c_int);

    fn gtk_box_new(orientation: c_int, spacing: c_int) -> *mut GtkWidget;
    fn gtk_box_pack_start(
        box_widget: *mut GtkBox,
        child: *mut GtkWidget,
        expand: c_int,
        fill: c_int,
        padding: u32,
    );

    fn gtk_container_add(container: *mut GtkContainer, widget: *mut GtkWidget);
    fn gtk_container_set_border_width(container: *mut GtkContainer, border_width: u32);

    fn gtk_button_new_with_label(label: *const c_char) -> *mut GtkWidget;
    fn gtk_entry_new() -> *mut GtkWidget;
    fn gtk_entry_set_text(entry: *mut GtkEntry, text: *const c_char);
    fn gtk_entry_get_text(entry: *mut GtkEntry) -> *const c_char;

    fn gtk_label_new(str_text: *const c_char) -> *mut GtkWidget;
    fn gtk_label_set_text(label: *mut GtkLabel, str_text: *const c_char);
    fn gtk_widget_set_halign(widget: *mut GtkWidget, align: c_int);

    fn gtk_drawing_area_new() -> *mut GtkWidget;
    fn gtk_widget_add_events(widget: *mut GtkWidget, events: c_int);
    fn gtk_widget_queue_draw(widget: *mut GtkWidget);
    fn gtk_widget_get_allocated_width(widget: *mut GtkWidget) -> c_int;
    fn gtk_widget_get_allocated_height(widget: *mut GtkWidget) -> c_int;
    fn gtk_widget_show_all(widget: *mut GtkWidget);
}

#[link(name = "gdk-3")]
unsafe extern "C" {
    fn gdk_event_get_coords(event: *const GdkEvent, x_win: *mut c_double, y_win: *mut c_double) -> c_int;
    fn gdk_event_get_scroll_direction(event: *const GdkEvent, direction: *mut c_int) -> c_int;
    fn gdk_event_get_scroll_deltas(
        event: *const GdkEvent,
        delta_x: *mut c_double,
        delta_y: *mut c_double,
    ) -> c_int;
}

#[link(name = "gobject-2.0")]
unsafe extern "C" {
    fn g_signal_connect_data(
        instance: *mut c_void,
        detailed_signal: *const c_char,
        c_handler: GCallback,
        data: *mut c_void,
        destroy_data: *const c_void,
        connect_flags: c_int,
    ) -> c_ulong;
}

#[link(name = "cairo")]
unsafe extern "C" {
    fn cairo_set_source_rgb(cr: *mut CairoT, red: c_double, green: c_double, blue: c_double);
    fn cairo_rectangle(
        cr: *mut CairoT,
        x: c_double,
        y: c_double,
        width: c_double,
        height: c_double,
    );
    fn cairo_fill(cr: *mut CairoT);
    fn cairo_stroke(cr: *mut CairoT);
    fn cairo_set_line_width(cr: *mut CairoT, width: c_double);
    fn cairo_select_font_face(
        cr: *mut CairoT,
        family: *const c_char,
        slant: c_int,
        weight: c_int,
    );
    fn cairo_set_font_size(cr: *mut CairoT, size: c_double);
    fn cairo_move_to(cr: *mut CairoT, x: c_double, y: c_double);
    fn cairo_show_text(cr: *mut CairoT, utf8: *const c_char);
}

/// Linux penceresinin arayüz bileşenlerini ve ortak `BrowserState` durumunu tutan yapı:
struct LinuxGuiApp {
    window: usize,
    url_entry: usize,
    canvas: usize,
    status_label: usize,
    state: BrowserState,
}

static LINUX_APP: Mutex<Option<LinuxGuiApp>> = Mutex::new(None);

/// Rust metnini (`&str`) C dilinin anlayacağı sonu `\0` bitli `CString` yapısına çevirir.
fn to_cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new("").unwrap())
}

/// GTK sinyalini (örn: `"clicked"`, `"activate"`, `"draw"`) bir Rust fonksiyonuna bağlar.
unsafe fn connect_signal(instance: *mut GtkWidget, signal_name: &str, callback: GCallback) {
    let c_sig = to_cstr(signal_name);
    unsafe {
        g_signal_connect_data(instance, c_sig.as_ptr(), callback, null_mut(), null(), 0);
    }
}

/// Pencere başlığını, adres çubuğunu, durum çubuğunu günceller ve tuvali yeniden çizdirir.
fn arayuzu_guncelle(app: &LinuxGuiApp) {
    let title_c = to_cstr(&app.state.window_title());
    let url_c = to_cstr(&app.state.current_url);
    let status_c = to_cstr(&app.state.status_message);

    unsafe {
        gtk_window_set_title(app.window as *mut GtkWindow, title_c.as_ptr());
        gtk_entry_set_text(app.url_entry as *mut GtkEntry, url_c.as_ptr());
        gtk_label_set_text(app.status_label as *mut GtkLabel, status_c.as_ptr());
        gtk_widget_queue_draw(app.canvas as *mut GtkWidget);
    }
}

/// Verilen adrese gider ve Linux penceresini yeniler.
fn adres_yukle(hedef: &str, gecmise_ekle: bool) {
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        let alloc_w = unsafe { gtk_widget_get_allocated_width(app.canvas as *mut GtkWidget) };
        if alloc_w > 300 {
            app.state.viewport_width = alloc_w as u32;
        }
        app.state.navigate(hedef, gecmise_ekle);
        arayuzu_guncelle(app);
    }
}

// ============================================================================
// GTK OLAY GERİ ÇAĞIRIMLARI (EVENT CALLBACKS)
// ============================================================================

/// Pencere kapatıldığında (`destroy`) GTK ana döngüsünü sonlandırır.
unsafe extern "C" fn on_window_destroy(_widget: *mut GtkWidget, _data: *mut c_void) {
    unsafe {
        gtk_main_quit();
    }
}

/// `Git ->` butonuna basıldığında veya adres çubuğunda `Enter` tuşuna basıldığında çalışır.
unsafe extern "C" fn on_navigate_triggered(_widget: *mut GtkWidget, _data: *mut c_void) {
    let girilen_adres = if let Ok(guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_ref()
    {
        unsafe {
            let ptr = gtk_entry_get_text(app.url_entry as *mut GtkEntry);
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

/// `<-` (Geri) butonuna basıldığında çalışır.
unsafe extern "C" fn on_back_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
        && app.state.go_back()
    {
        arayuzu_guncelle(app);
    }
}

/// `->` (İleri) butonuna basıldığında çalışır.
unsafe extern "C" fn on_forward_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
        && app.state.go_forward()
    {
        arayuzu_guncelle(app);
    }
}

/// `Yenile` butonuna basıldığında çalışır.
unsafe extern "C" fn on_reload_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    if let Ok(mut guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_mut()
    {
        app.state.reload();
        arayuzu_guncelle(app);
    }
}

/// `Ana Sayfa` butonuna basıldığında çalışır.
unsafe extern "C" fn on_home_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    adres_yukle("http://anka.local/hosgeldiniz", true);
}

/// `CERN` butonuna basıldığında çalışır.
unsafe extern "C" fn on_cern_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    adres_yukle("http://info.cern.ch", true);
}

/// `HTTPS Example` butonuna basıldığında çalışır.
unsafe extern "C" fn on_example_clicked(_widget: *mut GtkWidget, _data: *mut c_void) {
    adres_yukle("https://example.com", true);
}

/// Tuval (`GtkDrawingArea`) üzerine fare ile tıklandığında (`button-press-event`)
/// tıklanan koordinatta bir `<a href="...">` linki varsa o adrese gider!
unsafe extern "C" fn on_canvas_button_press(
    _widget: *mut GtkWidget,
    event: *const GdkEvent,
    _data: *mut c_void,
) -> c_int {
    let mut x: c_double = 0.0;
    let mut y: c_double = 0.0;
    let ok = unsafe { gdk_event_get_coords(event, &mut x, &mut y) };
    if ok == 0 {
        return 0;
    }

    let hedef_link = if let Ok(guard) = LINUX_APP.lock()
        && let Some(app) = guard.as_ref()
    {
        app.state.link_at(x as i32, y as i32)
    } else {
        None
    };

    if let Some(yeni_url) = hedef_link {
        adres_yukle(&yeni_url, true);
    }

    0
}

/// Tuval üzerinde fare tekerleği çevrildiğinde (`scroll-event`) sayfayı aşağı/yukarı kaydırır.
unsafe extern "C" fn on_canvas_scroll(
    widget: *mut GtkWidget,
    event: *const GdkEvent,
    _data: *mut c_void,
) -> c_int {
    let mut dir: c_int = -1;
    let mut delta_x: c_double = 0.0;
    let mut delta_y: c_double = 0.0;

    let scroll_step = unsafe {
        if gdk_event_get_scroll_direction(event, &mut dir) != 0 {
            match dir {
                GDK_SCROLL_UP => -40,
                GDK_SCROLL_DOWN => 40,
                _ => 0,
            }
        } else if gdk_event_get_scroll_deltas(event, &mut delta_x, &mut delta_y) != 0 {
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
            gtk_widget_queue_draw(widget);
        }
    }

    0
}

/// CAIRO 2D ÇİZİM MOTORU (`draw` sinyali):
/// `anka-paint` modülümüzden gelen `PaintCommand` listesini Linux penceresinin tuvaline çizer!
unsafe extern "C" fn on_canvas_draw(
    widget: *mut GtkWidget,
    cr: *mut CairoT,
    _data: *mut c_void,
) -> c_int {
    unsafe {
        let width = gtk_widget_get_allocated_width(widget) as c_double;
        let height = gtk_widget_get_allocated_height(widget) as c_double;

        // 1. Sayfa arka planını bembeyaz boya
        cairo_set_source_rgb(cr, 1.0, 1.0, 1.0);
        cairo_rectangle(cr, 0.0, 0.0, width, height);
        cairo_fill(cr);

        let font_family = to_cstr("Sans");

        if let Ok(guard) = LINUX_APP.lock()
            && let Some(app) = guard.as_ref()
        {
            for komut in &app.state.display_list {
                let draw_x = (komut.x as c_double) + 8.0;
                let draw_y = (komut.y as i32 - app.state.scroll_y) as c_double;
                let box_w = (komut.width as c_double).min((width - 24.0).max(100.0));
                let box_h = komut.height as c_double;

                // Ekranın görünür alanında değilse çizimi atla (Performans optimizasyonu)
                if draw_y + box_h < 0.0 || draw_y > height {
                    continue;
                }

                // 2. Eğer kutunun CSS arka plan rengi (`background-color`) varsa boya
                if let Some(bg) = &komut.bg_color {
                    cairo_set_source_rgb(
                        cr,
                        bg.r as c_double / 255.0,
                        bg.g as c_double / 255.0,
                        bg.b as c_double / 255.0,
                    );
                    cairo_rectangle(cr, draw_x - 4.0, draw_y - 2.0, box_w, box_h);
                    cairo_fill(cr);
                }

                // 3. Eğer kutunun CSS çerçevesi (`border`) varsa etrafına 1px çerçeve çiz
                if let Some(border) = &komut.border_color {
                    cairo_set_source_rgb(
                        cr,
                        border.r as c_double / 255.0,
                        border.g as c_double / 255.0,
                        border.b as c_double / 255.0,
                    );
                    cairo_set_line_width(cr, 1.0);
                    cairo_rectangle(cr, draw_x - 4.0, draw_y - 2.0, box_w, box_h);
                    cairo_stroke(cr);
                }

                // 4. Metni (veya link alt çizgisini) çiz
                if !komut.text.is_empty() {
                    let weight = if matches!(komut.tag.as_str(), "h1" | "h2" | "h3") {
                        CAIRO_FONT_WEIGHT_BOLD
                    } else {
                        CAIRO_FONT_WEIGHT_NORMAL
                    };

                    cairo_select_font_face(
                        cr,
                        font_family.as_ptr(),
                        CAIRO_FONT_SLANT_NORMAL,
                        weight,
                    );
                    cairo_set_font_size(cr, komut.font_size as c_double);

                    cairo_set_source_rgb(
                        cr,
                        komut.color.r as c_double / 255.0,
                        komut.color.g as c_double / 255.0,
                        komut.color.b as c_double / 255.0,
                    );

                    // Cairo'da metinler taban çizgisinden (baseline) çizildiği için font_size kadar aşağı kaydırıyoruz
                    let baseline_y = draw_y + (komut.font_size as c_double);
                    cairo_move_to(cr, draw_x + 4.0, baseline_y);

                    let c_text = to_cstr(&komut.text);
                    cairo_show_text(cr, c_text.as_ptr());

                    // Eğer `<a>` link etiketi ise altına ince bir çizgi (underline) çek
                    if komut.tag == "a" {
                        cairo_set_line_width(cr, 1.0);
                        let line_w = ((komut.text.chars().count() as u32)
                            * (komut.font_size / 2).max(7))
                            as c_double;
                        cairo_rectangle(cr, draw_x + 4.0, baseline_y + 2.0, line_w, 1.0);
                        cairo_fill(cr);
                    }
                }
            }
        }
    }
    0
}

/// Linux masaüstü ortamında (Wayland veya X11) GTK3 penceresini başlatmayı dener.
/// Eğer masaüstü ekranı varsa pencereyi açıp `true` döner;
/// eğer sunucu/CI (ekransız terminal) ortamındaysa `false` döner (böylece terminal önizlemesi çalışır).
pub fn pencereyi_baslat(baslangic_adresi: &str) -> bool {
    unsafe {
        let mut argc: c_int = 0;
        let mut argv: *mut *mut c_char = null_mut();
        // `gtk_init_check`: Grafik ekran (Wayland/X11) yoksa çökmez, 0 (false) döndürür!
        if gtk_init_check(&mut argc, &mut argv) == 0 {
            return false;
        }

        let state = BrowserState::new(baslangic_adresi, 920);

        // 1. Ana Pencereyi Oluştur
        let window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
        let title_c = to_cstr(&state.window_title());
        gtk_window_set_title(window as *mut GtkWindow, title_c.as_ptr());
        gtk_window_set_default_size(window as *mut GtkWindow, 960, 680);
        connect_signal(window, "destroy", on_window_destroy as GCallback);

        // 2. Dikey Ana Kutu (Üstte Araç Çubuğu, Ortada Web Tuvali, Altta Durum Çubuğu)
        let vbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
        gtk_container_add(window as *mut GtkContainer, vbox);

        // 3. Üst Araç Çubuğu (Yatay Kutu)
        let toolbar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 6);
        gtk_container_set_border_width(toolbar as *mut GtkContainer, 6);
        gtk_box_pack_start(vbox as *mut GtkBox, toolbar, 0, 1, 0);

        let btn_back = gtk_button_new_with_label(to_cstr("<-").as_ptr());
        let btn_forward = gtk_button_new_with_label(to_cstr("->").as_ptr());
        let btn_reload = gtk_button_new_with_label(to_cstr("Yenile").as_ptr());
        let btn_home = gtk_button_new_with_label(to_cstr("Ana Sayfa").as_ptr());
        let btn_cern = gtk_button_new_with_label(to_cstr("CERN").as_ptr());
        let btn_example = gtk_button_new_with_label(to_cstr("HTTPS Example").as_ptr());
        let url_entry = gtk_entry_new();
        let btn_go = gtk_button_new_with_label(to_cstr("Git ->").as_ptr());

        gtk_entry_set_text(
            url_entry as *mut GtkEntry,
            to_cstr(&state.current_url).as_ptr(),
        );

        gtk_box_pack_start(toolbar as *mut GtkBox, btn_back, 0, 0, 0);
        gtk_box_pack_start(toolbar as *mut GtkBox, btn_forward, 0, 0, 0);
        gtk_box_pack_start(toolbar as *mut GtkBox, btn_reload, 0, 0, 0);
        gtk_box_pack_start(toolbar as *mut GtkBox, btn_home, 0, 0, 0);
        gtk_box_pack_start(toolbar as *mut GtkBox, btn_cern, 0, 0, 0);
        gtk_box_pack_start(toolbar as *mut GtkBox, btn_example, 0, 0, 0);
        gtk_box_pack_start(toolbar as *mut GtkBox, url_entry, 1, 1, 0);
        gtk_box_pack_start(toolbar as *mut GtkBox, btn_go, 0, 0, 0);

        connect_signal(btn_back, "clicked", on_back_clicked as GCallback);
        connect_signal(btn_forward, "clicked", on_forward_clicked as GCallback);
        connect_signal(btn_reload, "clicked", on_reload_clicked as GCallback);
        connect_signal(btn_home, "clicked", on_home_clicked as GCallback);
        connect_signal(btn_cern, "clicked", on_cern_clicked as GCallback);
        connect_signal(btn_example, "clicked", on_example_clicked as GCallback);
        connect_signal(btn_go, "clicked", on_navigate_triggered as GCallback);
        connect_signal(url_entry, "activate", on_navigate_triggered as GCallback);

        // 4. Orta Web Sayfası Çizim Tuvali (`GtkDrawingArea`)
        let canvas = gtk_drawing_area_new();
        gtk_widget_add_events(
            canvas,
            GDK_BUTTON_PRESS_MASK | GDK_SCROLL_MASK | GDK_SMOOTH_SCROLL_MASK,
        );
        gtk_box_pack_start(vbox as *mut GtkBox, canvas, 1, 1, 0);

        connect_signal(canvas, "draw", on_canvas_draw as GCallback);
        connect_signal(
            canvas,
            "button-press-event",
            on_canvas_button_press as GCallback,
        );
        connect_signal(canvas, "scroll-event", on_canvas_scroll as GCallback);

        // 5. Alt Durum Çubuğu (`GtkLabel`)
        let status_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
        gtk_container_set_border_width(status_box as *mut GtkContainer, 6);
        let status_label = gtk_label_new(to_cstr(&state.status_message).as_ptr());
        gtk_widget_set_halign(status_label, 1); // Sola hizala (GTK_ALIGN_START = 1)
        gtk_box_pack_start(status_box as *mut GtkBox, status_label, 1, 1, 4);
        gtk_box_pack_start(vbox as *mut GtkBox, status_box, 0, 1, 0);

        if let Ok(mut guard) = LINUX_APP.lock() {
            *guard = Some(LinuxGuiApp {
                window: window as usize,
                url_entry: url_entry as usize,
                canvas: canvas as usize,
                status_label: status_label as usize,
                state,
            });
        }

        // 6. Tüm bileşenleri ekranda göster ve olay döngüsünü başlat!
        gtk_widget_show_all(window);
        gtk_main();
        true
    }
}
