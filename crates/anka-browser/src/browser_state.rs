// Anka Browser - İşletim Sisteminden Bağımsız Ortak Tarayıcı Durumu (BrowserState)
// Çok Süreçli (Multi-Process) Sekme İzolasyonu, Anka Kalkan, Kalıcı Yer İmleri,
// Canlı JS `onclick` ve Etkileşimli HTML `<form>` / `<input>` Yönetimi (v0.3.0)

use std::collections::HashMap;
use std::path::PathBuf;

use anka_paint::PaintCommand;
use anka_url::Url;

use crate::normalize_address;
use crate::process_ipc::{RenderSnapshot, TabWorkerClient};

/// Kullanıcının web sayfası tuvaline tıkladığında tetiklenebilecek eylem türleri:
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickAction {
    /// Bir `<a href="...">` linkine veya `<form>` gönder butonuna tıklandı -> yeni adrese git!
    Navigate(String),
    /// Bir `<button onclick="...">` butonuna veya `<input>` kutusuna tıklandı -> ekran yeniden çizilmeli!
    Redraw,
    /// Boş bir yere tıklandı
    None,
}

/// Tek bir tarayıcı sekmesinin (Tab) bağımsız canlı durumunu tutan veri yapısı:
/// Her sekme kendi izole `--renderer-worker` işçi sürecine (`worker`) sahiptir!
pub struct TabState {
    /// Bu sekmede açık olan sayfanın tam adresi
    pub current_url: String,
    /// İzole işçi süreçten gelen son çizim ve durum anlık görüntüsü
    pub page: RenderSnapshot,
    /// Bu sekmeye ait izole OS alt süreci (Renderer Process) istemcisi
    pub worker: TabWorkerClient,
    /// Bu sekmenin kendi Geri (`<-`) geçmişi yığını
    pub history: Vec<String>,
    /// Bu sekmenin kendi İleri (`->`) geçmişi yığını
    pub forward_history: Vec<String>,
    /// Bu sekmenin dikey kaydırma miktarı (piksel)
    pub scroll_y: i32,
    /// Şu an klavye odağında (focus) olan `<input name="...">` alanının adı (varsa)
    pub focused_input: Option<String>,
    /// Bu sekmedeki `<input name="...">` kutularına kullanıcının yazdığı metinler
    pub input_values: HashMap<String, String>,
}

/// Kullanıcının kaydettiği tek bir Yer İmi (Bookmark) kaydı:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

/// Tüm açık sekmeleri (`tabs`), aktif sekme indeksini (`active_tab`) ve
/// diskte kalıcı olarak saklanan yer imlerini (`bookmarks`) yöneten ana tarayıcı durumu:
pub struct BrowserState {
    pub tabs: Vec<TabState>,
    pub active_tab: usize,
    pub bookmarks: Vec<Bookmark>,
    pub viewport_width: u32,
    /// Anka Kalkan (Reklam & İzleyici Engelleyici) açık mı?
    pub shield_enabled: bool,
    /// Yer imlerinin kaydedileceği dosya yolu (`None` ise disk kaydı yapılmaz — test izolasyonu için)
    bookmarks_file: Option<PathBuf>,
}

/// İşletim sistemine uygun standart kalıcı ayar dosyası yolunu döndürür:
/// - Linux/Unix: `~/.config/anka-browser/bookmarks.tsv`
/// - Windows: `%APPDATA%\anka-browser\bookmarks.tsv`
pub fn default_bookmarks_path() -> Option<PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA")
        && !appdata.trim().is_empty()
    {
        return Some(PathBuf::from(appdata).join("anka-browser").join("bookmarks.tsv"));
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return Some(
            PathBuf::from(home)
                .join(".config")
                .join("anka-browser")
                .join("bookmarks.tsv"),
        );
    }
    None
}

/// Yer imlerini basit, insan tarafından okunabilir TSV (`baslik\turl`) metnine dönüştürür.
pub fn serialize_bookmarks(bookmarks: &[Bookmark]) -> String {
    let mut out = String::new();
    for bm in bookmarks {
        let clean_title = bm.title.replace(['\t', '\r', '\n'], " ");
        let clean_url = bm.url.replace(['\t', '\r', '\n'], "");
        out.push_str(&format!("{clean_title}\t{clean_url}\n"));
    }
    out
}

/// Diskten okunan TSV metnini `Vec<Bookmark>` listesine dönüştürür.
pub fn deserialize_bookmarks(raw: &str) -> Vec<Bookmark> {
    let mut list = Vec::new();
    for line in raw.lines() {
        if let Some((title, url)) = line.split_once('\t') {
            let t = title.trim();
            let u = url.trim();
            if !u.is_empty() {
                list.push(Bookmark {
                    title: if t.is_empty() { u.to_string() } else { t.to_string() },
                    url: u.to_string(),
                });
            }
        }
    }
    list
}

/// Form gönderiminde arama metnindeki boşluk ve özel karakterleri URL güvenli biçime çevirir:
/// Örn: `"anka kusu"` -> `"anka+kusu"`
pub fn percent_encode_query(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn default_bookmarks() -> Vec<Bookmark> {
    vec![
        Bookmark {
            title: "Anka Ana Sayfa".to_string(),
            url: "http://anka.local/hosgeldiniz".to_string(),
        },
        Bookmark {
            title: "HTTPS Example.com".to_string(),
            url: "https://example.com".to_string(),
        },
        Bookmark {
            title: "CERN (1991 İlk Web Sitesi)".to_string(),
            url: "http://info.cern.ch".to_string(),
        },
        Bookmark {
            title: "Vikipedi Türkçe".to_string(),
            url: "https://tr.wikipedia.org/wiki/Anasayfa".to_string(),
        },
    ]
}

impl BrowserState {
    /// Başlangıç adresi ve pencere genişliği ile ilk sekmeyi ve diskteki kalıcı yer imlerini yükler.
    pub fn new(initial_address: &str, viewport_width: u32) -> Self {
        let storage_path = if cfg!(test) {
            None
        } else {
            default_bookmarks_path()
        };
        Self::with_bookmarks_path(initial_address, viewport_width, storage_path)
    }

    /// Belirli bir yer imi dosya yoluyla (gerçek disk veya test geçici dosyası) `BrowserState` başlatır.
    pub fn with_bookmarks_path(
        initial_address: &str,
        viewport_width: u32,
        bookmarks_file: Option<PathBuf>,
    ) -> Self {
        let bookmarks = bookmarks_file
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|raw| deserialize_bookmarks(&raw))
            .filter(|list| !list.is_empty())
            .unwrap_or_else(default_bookmarks);

        let shield_enabled = true;
        let mut temiz_url = normalize_address(initial_address);
        if shield_enabled {
            let (cleaned, _) = anka_net::shield::strip_tracking_params(&temiz_url);
            temiz_url = cleaned;
        }
        let mut worker = TabWorkerClient::new(!cfg!(test));
        let ilk_sayfa = worker.load_url(initial_address, viewport_width, shield_enabled);

        Self {
            tabs: vec![TabState {
                current_url: temiz_url,
                page: ilk_sayfa,
                worker,
                history: Vec::new(),
                forward_history: Vec::new(),
                scroll_y: 0,
                focused_input: None,
                input_values: HashMap::new(),
            }],
            active_tab: 0,
            bookmarks,
            viewport_width,
            shield_enabled,
            bookmarks_file,
        }
    }

    /// Güncel yer imlerini diskteki `bookmarks.tsv` dosyasına kalıcı olarak yazar.
    fn persist_bookmarks(&self) {
        if let Some(path) = &self.bookmarks_file {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let content = serialize_bookmarks(&self.bookmarks);
            let _ = std::fs::write(path, content);
        }
    }

    pub fn active(&self) -> &TabState {
        &self.tabs[self.active_tab]
    }

    fn active_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.active_tab]
    }

    pub fn current_url(&self) -> &str {
        &self.active().current_url
    }

    pub fn scroll_y(&self) -> i32 {
        self.active().scroll_y
    }

    pub fn focused_input(&self) -> Option<&str> {
        self.active().focused_input.as_deref()
    }

    /// Bir `<input>` kutusunun ekranda gösterilecek canlı değerini döndürür.
    pub fn input_value(&self, name: &str) -> Option<&str> {
        self.active().input_values.get(name).map(String::as_str)
    }

    pub fn status_message(&self) -> String {
        let yildiz = if self.is_current_bookmarked() {
            "★ Yer Imi"
        } else {
            "☆"
        };
        let active = self.active();
        let pid_tag = if active.page.crashed {
            format!("PID:{} (COKTU)", active.page.pid)
        } else {
            format!("PID:{}", active.page.pid)
        };
        format!(
            "[Sekme {}/{} | {} | {}] {}",
            self.active_tab + 1,
            self.tabs.len(),
            pid_tag,
            yildiz,
            active.page.status_message
        )
    }

    pub fn display_list(&self) -> &[PaintCommand] {
        &self.active().page.display_list
    }

    pub fn page_bg(&self) -> Option<anka_paint::RgbColor> {
        self.active().page.page_bg
    }

    pub fn window_title(&self) -> String {
        let tab_info = format!("[Sekme {}/{}]", self.active_tab + 1, self.tabs.len());
        match &self.active().page.page_title {
            Some(t) => format!("{t} {tab_info} — Anka Browser v0.3.0"),
            None => format!("Anka Browser v0.3.0 {tab_info} — Telek Engine"),
        }
    }

    pub fn shield_button_label(&self) -> String {
        if self.shield_enabled {
            let count = self.active().page.shield_blocked;
            if count > 0 {
                format!("Kalkan: Acik ({count})")
            } else {
                "Kalkan: Acik".to_string()
            }
        } else {
            "Kalkan: Kapali".to_string()
        }
    }

    /// Anka Kalkan korumasını açıp kapatır ve aktif sekmeyi yeni kurala göre yeniden yükler.
    pub fn toggle_shield(&mut self) -> bool {
        self.shield_enabled = !self.shield_enabled;
        self.reload();
        self.shield_enabled
    }

    pub fn tab_label(&self, index: usize) -> String {
        let Some(tab) = self.tabs.get(index) else {
            return format!("Sekme {}", index + 1);
        };
        let ham_baslik = tab
            .page
            .page_title
            .as_deref()
            .unwrap_or(tab.current_url.as_str());
        let kisa: String = ham_baslik.chars().take(16).collect();
        if index == self.active_tab {
            format!("● {}: {}", index + 1, kisa)
        } else {
            format!("{}: {}", index + 1, kisa)
        }
    }

    pub fn new_tab(&mut self, initial_address: &str) -> usize {
        let mut clean_url = normalize_address(initial_address);
        if self.shield_enabled {
            let (cleaned, _) = anka_net::shield::strip_tracking_params(&clean_url);
            clean_url = cleaned;
        }
        let mut worker = TabWorkerClient::new(!cfg!(test));
        let sayfa = worker.load_url(initial_address, self.viewport_width, self.shield_enabled);
        self.tabs.push(TabState {
            current_url: clean_url,
            page: sayfa,
            worker,
            history: Vec::new(),
            forward_history: Vec::new(),
            scroll_y: 0,
            focused_input: None,
            input_values: HashMap::new(),
        });
        self.active_tab = self.tabs.len() - 1;
        self.active_tab
    }

    pub fn switch_tab(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab = index;
            true
        } else {
            false
        }
    }

    pub fn close_active_tab(&mut self) {
        if self.tabs.len() <= 1 {
            self.navigate("http://anka.local/hosgeldiniz", false);
            return;
        }
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    pub fn is_current_bookmarked(&self) -> bool {
        let current = self.current_url();
        self.bookmarks.iter().any(|b| b.url == current)
    }

    /// Aktif sayfayı Yer İmlerine ekler/çıkarır ve değişikliği ANINDA diske kaydeder!
    pub fn toggle_bookmark(&mut self) -> bool {
        let current = self.current_url().to_string();
        if current.is_empty() || current == "http://anka.local/yer-imleri" {
            return false;
        }

        let added = if let Some(pos) = self.bookmarks.iter().position(|b| b.url == current) {
            self.bookmarks.remove(pos);
            false
        } else {
            let title = self
                .active()
                .page
                .page_title
                .clone()
                .unwrap_or_else(|| current.clone());
            self.bookmarks.push(Bookmark {
                title,
                url: current,
            });
            true
        };

        self.persist_bookmarks();
        added
    }

    fn build_bookmarks_http_response(&self) -> String {
        let mut html_items = String::new();
        for (i, bm) in self.bookmarks.iter().enumerate() {
            html_items.push_str(&format!(
                "<div class=\"kart\"><p>{}. {}</p><a class=\"pill\" href=\"{}\">{}</a></div>",
                i + 1,
                bm.title,
                bm.url,
                bm.url
            ));
        }
        if html_items.is_empty() {
            html_items.push_str(
                "<div id=\"duyuru\"><p>Henuz hic yer imi kaydetmediniz. Bir sayfadayken ustteki '* Yer Imi' butonuna basarak ekleyebilirsiniz.</p></div>",
            );
        }

        format!(
            concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: text/html\r\n\r\n",
                "<html><head><title>Yer Imlerim</title><style>",
                "h1 {{ color: #1e40af; background-color: #eff6ff; border: 1px solid #93c5fd; font-size: 28px; }}",
                "h2 {{ color: #0f766e; font-size: 20px; }}",
                "p  {{ color: #1e293b; font-size: 16px; }}",
                ".pill {{ color: #1d4ed8; background-color: #dbeafe; border: 1px solid #60a5fa; font-size: 15px; }}",
                ".kart {{ color: #065f46; background-color: #ecfdf5; border: 1px solid #6ee7b7; padding: 8px; font-size: 16px; }}",
                "#duyuru {{ color: #9a3412; background-color: #ffedd5; border: 1px solid #fdba74; padding: 8px; font-size: 16px; }}",
                "</style></head><body>",
                "<h1>Anka Browser — Kalici Yer Imlerim ({} Kayit)</h1>",
                "<nav>",
                "<a class=\"pill\" href=\"http://anka.local/hosgeldiniz\">Ana Sayfaya Don</a>",
                "</nav>",
                "<hr>",
                "{}",
                "</body></html>"
            ),
            self.bookmarks.len(),
            html_items
        )
    }

    pub fn navigate(&mut self, raw_address: &str, push_history: bool) {
        let mut clean_url = normalize_address(raw_address);
        if clean_url.is_empty() {
            return;
        }
        if self.shield_enabled {
            let (cleaned, _) = anka_net::shield::strip_tracking_params(&clean_url);
            clean_url = cleaned;
        }

        let current_url_before = self.active().current_url.clone();
        if push_history && !current_url_before.is_empty() && current_url_before != clean_url {
            let tab = self.active_mut();
            tab.history.push(current_url_before);
            tab.forward_history.clear();
        }

        let viewport_w = self.viewport_width;
        let shield_on = self.shield_enabled;
        let loaded = if clean_url == "http://anka.local/yer-imleri" {
            let raw_http = self.build_bookmarks_http_response();
            self.active_mut()
                .worker
                .load_html(&clean_url, &raw_http, viewport_w, shield_on)
        } else {
            self.active_mut()
                .worker
                .load_url(raw_address, viewport_w, shield_on)
        };

        let tab = self.active_mut();
        tab.current_url = clean_url;
        tab.page = loaded;
        tab.scroll_y = 0;
        tab.focused_input = None;
        tab.input_values.clear();
    }

    pub fn go_back(&mut self) -> bool {
        let prev_opt = self.active_mut().history.pop();
        if let Some(prev_url) = prev_opt {
            let current = self.active().current_url.clone();
            self.active_mut().forward_history.push(current);
            self.navigate(&prev_url, false);
            true
        } else {
            false
        }
    }

    pub fn go_forward(&mut self) -> bool {
        let next_opt = self.active_mut().forward_history.pop();
        if let Some(next_url) = next_opt {
            let current = self.active().current_url.clone();
            self.active_mut().history.push(current);
            self.navigate(&next_url, false);
            true
        } else {
            false
        }
    }

    pub fn reload(&mut self) {
        let current = self.current_url().to_string();
        self.navigate(&current, false);
    }

    pub fn scroll_by(&mut self, delta_y: i32) {
        let tab = self.active_mut();
        tab.scroll_y = (tab.scroll_y + delta_y).max(0);
    }

    /// Odaklı bir `<input>` kutusuna tek bir karakter yazar.
    pub fn type_char(&mut self, ch: char) -> bool {
        let Some(name) = self.active().focused_input.clone() else {
            return false;
        };
        let entry = self.active_mut().input_values.entry(name).or_default();
        entry.push(ch);
        true
    }

    /// Odaklı `<input>` kutusundan son karakteri siler (`Backspace`).
    pub fn backspace_input(&mut self) -> bool {
        let Some(name) = self.active().focused_input.clone() else {
            return false;
        };
        if let Some(val) = self.active_mut().input_values.get_mut(&name) {
            val.pop();
        }
        true
    }

    /// Verilen `form_action` adresine sahip HTML `<form>` elemanını, içindeki `<input>`
    /// değerlerini `?name=encoded_value` sorgu parametresi olarak ekleyip tam URL üretir!
    pub fn build_form_submit_url(&self, action: &str) -> Option<String> {
        let tab = self.active();
        let mut params = Vec::new();
        for komut in &tab.page.display_list {
            if let Some(name) = &komut.input_name
                && komut.form_action.as_deref() == Some(action)
            {
                let val = tab
                    .input_values
                    .get(name)
                    .map(String::as_str)
                    .unwrap_or("");
                params.push(format!(
                    "{}={}",
                    percent_encode_query(name),
                    percent_encode_query(val)
                ));
            }
        }

        let target_raw = if action.is_empty() {
            tab.current_url.as_str()
        } else {
            action
        };
        let base_url = Url::parse(&tab.current_url).ok()?;
        let resolved = base_url.resolve(target_raw).ok()?;
        let port_part = resolved
            .port
            .map(|p| format!(":{p}"))
            .unwrap_or_default();
        let query_joined = params.join("&");
        let full_query = match (&resolved.query, query_joined.is_empty()) {
            (Some(existing), false) => format!("?{existing}&{query_joined}"),
            (Some(existing), true) => format!("?{existing}"),
            (None, false) => format!("?{query_joined}"),
            (None, true) => String::new(),
        };

        Some(format!(
            "{}://{}{}{}{}",
            resolved.scheme, resolved.host, port_part, resolved.path, full_query
        ))
    }

    /// Odaklı `<input>` kutusunda `Enter` tuşuna basıldığında bağlı olduğu formu gönderir!
    pub fn submit_focused_form(&self) -> Option<String> {
        let focused = self.active().focused_input.as_deref()?;
        let action = self
            .active()
            .page
            .display_list
            .iter()
            .find(|k| k.input_name.as_deref() == Some(focused))
            .and_then(|k| k.form_action.as_deref())?;
        self.build_form_submit_url(action)
    }

    #[allow(dead_code)]
    pub fn is_interactive_at(&self, canvas_x: i32, canvas_y: i32) -> bool {
        let tab = self.active();
        tab.page.display_list.iter().any(|komut| {
            if komut.href.is_some()
                || komut.onclick.is_some()
                || komut.input_name.is_some()
                || komut.is_submit
            {
                let draw_x = komut.x as i32 + 4;
                let draw_right = draw_x + komut.width as i32;
                let draw_y = komut.y as i32 - tab.scroll_y;
                let draw_bottom = draw_y + komut.height as i32;
                canvas_x >= draw_x
                    && canvas_x <= draw_right
                    && canvas_y >= draw_y
                    && canvas_y <= draw_bottom
            } else {
                false
            }
        })
    }

    /// Tuval üzerindeki `(canvas_x, canvas_y)` tıklamasını işler:
    /// 1. `<input>` kutusu -> Odaklar (`focused_input = Some(name)`) ve `ClickAction::Redraw` döner.
    /// 2. `<input type="submit">` / `<form><button>` -> Formu GET sorgusuyla gönderir (`ClickAction::Navigate`).
    /// 3. `<button onclick="...">` -> Canlı `anka-js` çalıştırır (`ClickAction::Redraw`).
    /// 4. `<a href="...">` -> Yeni adrese gider (`ClickAction::Navigate`).
    pub fn click_at(&mut self, canvas_x: i32, canvas_y: i32) -> ClickAction {
        let tab = self.active();
        let mut found_input: Option<String> = None;
        let mut found_submit_action: Option<String> = None;
        let mut found_onclick: Option<String> = None;
        let mut found_href: Option<String> = None;

        for komut in &tab.page.display_list {
            let draw_x = komut.x as i32 + 4;
            let draw_right = draw_x + komut.width as i32;
            let draw_y = komut.y as i32 - tab.scroll_y;
            let draw_bottom = draw_y + komut.height as i32;

            if canvas_x >= draw_x
                && canvas_x <= draw_right
                && canvas_y >= draw_y
                && canvas_y <= draw_bottom
            {
                if let Some(name) = &komut.input_name {
                    found_input = Some(name.clone());
                    break;
                }
                if let Some(js_code) = &komut.onclick {
                    found_onclick = Some(js_code.clone());
                    break;
                }
                if komut.is_submit {
                    found_submit_action =
                        Some(komut.form_action.clone().unwrap_or_default());
                    break;
                }
                if let Some(href) = &komut.href {
                    found_href = Some(href.clone());
                    break;
                }
            }
        }

        if let Some(input_name) = found_input {
            self.active_mut().focused_input = Some(input_name);
            return ClickAction::Redraw;
        }

        // Input dışına tıklandıysa klavye odağını kaldır
        self.active_mut().focused_input = None;

        if let Some(js_code) = found_onclick {
            let vw = self.viewport_width;
            let tab = self.active_mut();
            tab.page = tab.worker.eval_js(&js_code, vw);
            return ClickAction::Redraw;
        }

        if let Some(action) = found_submit_action
            && let Some(target_url) = self.build_form_submit_url(&action)
        {
            return ClickAction::Navigate(target_url);
        }

        if let Some(href) = found_href
            && let Ok(base_url) = Url::parse(&self.active().current_url)
            && let Ok(resolved) = base_url.resolve(&href)
        {
            let port_part = resolved
                .port
                .map(|p| format!(":{p}"))
                .unwrap_or_default();
            let query_part = resolved
                .query
                .map(|q| format!("?{q}"))
                .unwrap_or_default();
            return ClickAction::Navigate(format!(
                "{}://{}{}{}{}",
                resolved.scheme, resolved.host, port_part, resolved.path, query_part
            ));
        }

        ClickAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gezinme_gecmisi_geri_ve_ileri() {
        let mut state = BrowserState::new("http://anka.local/sayfa1", 860);
        assert_eq!(state.current_url(), "http://anka.local/sayfa1");
        assert!(state.active().history.is_empty());

        state.navigate("http://anka.local/sayfa2", true);
        assert_eq!(state.current_url(), "http://anka.local/sayfa2");
        assert_eq!(
            state.active().history,
            vec!["http://anka.local/sayfa1".to_string()]
        );

        assert!(state.go_back());
        assert_eq!(state.current_url(), "http://anka.local/sayfa1");
        assert_eq!(
            state.active().forward_history,
            vec!["http://anka.local/sayfa2".to_string()]
        );

        assert!(state.go_forward());
        assert_eq!(state.current_url(), "http://anka.local/sayfa2");
    }

    #[test]
    fn test_link_ve_canli_js_button_onclick_tiklama() {
        let mut state = BrowserState::new("http://anka.local/hosgeldiniz", 860);

        assert!(
            state
                .display_list()
                .iter()
                .any(|k| k.text.contains("Anka JS Hazir! Tiklama Sayisi: 0"))
        );

        let js_btn = state
            .display_list()
            .iter()
            .find(|k| k.onclick.as_deref() == Some("artir()"))
            .expect("Ana sayfada artir() JS butonu olmali")
            .clone();

        let action = state.click_at(js_btn.x as i32 + 10, js_btn.y as i32 + 5);
        assert_eq!(action, ClickAction::Redraw);

        assert!(
            state
                .display_list()
                .iter()
                .any(|k| k.text.contains("Tiklama Sayisi: 1"))
        );

        let ilk_link = state
            .display_list()
            .iter()
            .find(|k| k.href.as_deref() == Some("https://example.com"))
            .expect("Ana sayfada example.com linki olmali")
            .clone();
        assert_eq!(
            state.click_at(ilk_link.x as i32 + 10, ilk_link.y as i32 + 5),
            ClickAction::Navigate("https://example.com/".to_string())
        );
    }

    #[test]
    fn test_coklu_sekme_acma_gecis_ve_kapatma() {
        let mut state = BrowserState::new("http://anka.local/sekme1", 860);
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.active_tab, 0);

        let yeni_indeks = state.new_tab("http://anka.local/sekme2");
        assert_eq!(yeni_indeks, 1);
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.current_url(), "http://anka.local/sekme2");

        assert!(state.switch_tab(0));
        assert_eq!(state.current_url(), "http://anka.local/sekme1");

        state.close_active_tab();
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.current_url(), "http://anka.local/sekme2");
    }

    #[test]
    fn test_yer_imleri_ekleme_cikarma_ve_diske_kalicilik() {
        let tmp_file = std::env::temp_dir().join("anka_test_bookmarks_v028.tsv");
        let _ = std::fs::remove_file(&tmp_file);

        {
            let mut state = BrowserState::with_bookmarks_path(
                "http://anka.local/ozel-makale",
                860,
                Some(tmp_file.clone()),
            );
            assert!(!state.is_current_bookmarked());
            assert!(state.toggle_bookmark());
            assert!(state.is_current_bookmarked());
        }

        // Tarayıcı kapatılıp yeniden açıldığında kayıtlı yer imi diskten geri gelmeli!
        let state2 = BrowserState::with_bookmarks_path(
            "http://anka.local/ozel-makale",
            860,
            Some(tmp_file.clone()),
        );
        assert!(state2.is_current_bookmarked());
        let _ = std::fs::remove_file(&tmp_file);
    }

    #[test]
    fn test_html_form_input_yazma_ve_arama_gonderme() {
        let mut state = BrowserState::new("http://anka.local/hosgeldiniz", 860);
        let input_box = state
            .display_list()
            .iter()
            .find(|k| k.input_name.as_deref() == Some("search"))
            .expect("Ana sayfada Vikipedi arama kutusu olmali")
            .clone();

        // 1. Arama kutusuna tıkla (focus al)
        assert_eq!(
            state.click_at(input_box.x as i32 + 10, input_box.y as i32 + 5),
            ClickAction::Redraw
        );
        assert_eq!(state.focused_input(), Some("search"));

        // 2. Klavyeden "Anka kusu" yaz (son harfi silip tekrar ekle)
        for ch in "Anka kusx".chars() {
            assert!(state.type_char(ch));
        }
        assert!(state.backspace_input());
        assert!(state.type_char('u'));
        assert_eq!(state.input_value("search"), Some("Anka kusu"));

        // 3. Formu gönder -> `?search=Anka+kusu` URL'si üretilmeli!
        let submit_url = state
            .submit_focused_form()
            .expect("Form URL olusturulmali");
        assert_eq!(
            submit_url,
            "https://tr.wikipedia.org/w/index.php?search=Anka+kusu"
        );
    }

    #[test]
    fn test_anka_kalkan_reklam_engelleme_ve_ac_kapat() {
        let mut state = BrowserState::new("http://anka.local/kalkan", 860);
        assert!(state.shield_enabled);

        // Kalkan açıkken reklam kutusu gizlenmiş, 1x1 piksel + script + ?utm_source/?fbclid engellenmiş olmalı!
        assert!(state.active().page.shield_blocked >= 5);
        assert!(
            !state
                .display_list()
                .iter()
                .any(|b| b.text.contains("REKLAM ALANI"))
        );

        // Kalkanı kapat -> sayfa yenilenir ve reklam kutusu artık görünür hale gelir!
        assert!(!state.toggle_shield());
        assert_eq!(state.active().page.shield_blocked, 0);
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("REKLAM ALANI"))
        );
    }

    #[test]
    fn test_cok_surecli_sekme_cokme_izolasyonu_ve_kurtarma() {
        let mut state = BrowserState::new("http://anka.local/hosgeldiniz", 860);
        state.new_tab("http://anka.local/sayfa2");
        assert_eq!(state.tabs.len(), 2);

        // 2. sekmede çökme testi tetiklenince sadece 2. sekme çökmeli!
        state.navigate("http://anka.local/cokme-testi", true);
        assert!(state.active().page.crashed);
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("Coktu"))
        );

        // 1. sekmeye geç: Hiç etkilenmemiş ve canlı olmalı!
        assert!(state.switch_tab(0));
        assert!(!state.active().page.crashed);
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("Anka JS Hazir!"))
        );

        // Tekrar 2. sekmeye dönüp yeni bir adres açınca yeni işçi ile sekme kurtarılmalı!
        assert!(state.switch_tab(1));
        state.navigate("http://anka.local/hosgeldiniz", false);
        assert!(!state.active().page.crashed);
        assert!(state.active().page.pid > 0);
    }
}
