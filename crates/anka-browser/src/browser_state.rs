// Anka Browser - İşletim Sisteminden Bağımsız Ortak Tarayıcı Durumu (BrowserState)
// Çok Süreçli (Multi-Process) Sekme İzolasyonu, Anka Kalkan, Kalıcı Yer İmleri, Geçmiş & Çerezler,
// DevTools, Sayfa İçi Arama (Ctrl+F), Yakınlaştırma (Zoom) ve Canlı HTML `<form>` / `<textarea>` Yönetimi (v0.3.4)

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anka_net::cookies::CookieJar;
use anka_paint::PaintCommand;
use anka_url::Url;

use crate::process_ipc::{RenderSnapshot, TabWorkerClient};
use crate::{extract_form_field, html_escape, normalize_address};

/// Sekme yakınlaştırma (`Ctrl+` / `Ctrl-` / `Ctrl+0`) adım tablosu (%)
const ZOOM_LEVELS: &[u32] = &[50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200];

/// Kullanıcının web sayfası tuvaline tıkladığında tetiklenebilecek eylem türleri:
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickAction {
    /// Bir `<a href="...">` linkine veya GET `<form>` gönder butonuna tıklandı -> yeni adrese git!
    Navigate(String),
    /// Bir `method="POST"` `<form>` gönderildi -> gövdeyle POST isteği at!
    SubmitPost { url: String, body: String },
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
    /// Sekmeye özel yakınlaştırma oranı (%50 ..= %200, varsayılan %100)
    pub zoom_percent: u32,
    /// `Ctrl+F` sayfa içi arama çubuğu açık mı?
    pub find_open: bool,
    /// `Ctrl+F` arama sorgu metni
    pub find_query: String,
    /// Arama sorgusuyla eşleşen `display_list` komut indisleri
    pub find_matches: Vec<usize>,
    /// Aktif vurgulu eşleşmenin `find_matches` içindeki sırası
    pub find_active: usize,
}

/// Kullanıcının kaydettiği tek bir Yer İmi (Bookmark) kaydı:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

/// Ziyaret edilen tek bir sayfa geçmişi kaydı (`history.tsv`):
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub title: String,
    pub url: String,
    pub visited_epoch: u64,
}

const MAX_HISTORY_ENTRIES: usize = 200;

/// Tüm açık sekmeleri (`tabs`), aktif sekme indeksini (`active_tab`) ve
/// diskte kalıcı olarak saklanan yer imleri, çerezler ve gezinme geçmişini yöneten ana tarayıcı durumu:
pub struct BrowserState {
    pub tabs: Vec<TabState>,
    pub active_tab: usize,
    pub bookmarks: Vec<Bookmark>,
    pub history_entries: Vec<HistoryEntry>,
    pub viewport_width: u32,
    /// Anka Kalkan (Reklam & İzleyici Engelleyici) açık mı?
    pub shield_enabled: bool,
    /// Tüm sekmeler arasında paylaşılan oturum ve HTTP çerez kavanozu
    pub cookies: CookieJar,
    /// Yer imlerinin kaydedileceği dosya yolu (`None` ise disk kaydı yapılmaz — test izolasyonu için)
    bookmarks_file: Option<PathBuf>,
    /// Kalıcı çerezlerin kaydedileceği dosya yolu (`None` ise disk kaydı yapılmaz)
    cookies_file: Option<PathBuf>,
    /// Ziyaret geçmişinin kaydedileceği dosya yolu (`None` ise disk kaydı yapılmaz)
    history_file: Option<PathBuf>,
}

fn default_config_file(filename: &str) -> Option<PathBuf> {
    if let Ok(appdata) = std::env::var("APPDATA")
        && !appdata.trim().is_empty()
    {
        return Some(PathBuf::from(appdata).join("anka-browser").join(filename));
    }
    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return Some(
            PathBuf::from(home)
                .join(".config")
                .join("anka-browser")
                .join(filename),
        );
    }
    None
}

/// İşletim sistemine uygun standart kalıcı yer imleri dosyası yolunu döndürür:
pub fn default_bookmarks_path() -> Option<PathBuf> {
    default_config_file("bookmarks.tsv")
}

/// İşletim sistemine uygun standart kalıcı çerez dosyası yolunu döndürür:
pub fn default_cookies_path() -> Option<PathBuf> {
    default_config_file("cookies.tsv")
}

/// İşletim sistemine uygun standart kalıcı gezinme geçmişi dosyası yolunu döndürür:
pub fn default_history_path() -> Option<PathBuf> {
    default_config_file("history.tsv")
}

fn format_epoch_utc(epoch: u64) -> String {
    let days = (epoch / 86_400) as i64;
    let rem = epoch % 86_400;
    let hh = rem / 3_600;
    let mm = (rem % 3_600) / 60;

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(m <= 2);
    format!("{year:04}-{m:02}-{d:02} {hh:02}:{mm:02} UTC")
}

pub fn serialize_history(entries: &[HistoryEntry]) -> String {
    let mut out = String::new();
    for h in entries {
        let clean_title = h.title.replace(['\t', '\r', '\n'], " ");
        let clean_url = h.url.replace(['\t', '\r', '\n'], "");
        out.push_str(&format!("{}\t{}\t{}\n", h.visited_epoch, clean_title, clean_url));
    }
    out
}

pub fn deserialize_history(raw: &str) -> Vec<HistoryEntry> {
    let mut list = Vec::new();
    for line in raw.lines() {
        let mut it = line.splitn(3, '\t');
        let Some(ep_s) = it.next() else { continue };
        let Some(title) = it.next() else { continue };
        let Some(url) = it.next() else { continue };
        let Ok(visited_epoch) = ep_s.trim().parse::<u64>() else {
            continue;
        };
        let u = url.trim();
        if u.is_empty() {
            continue;
        }
        let t = title.trim();
        list.push(HistoryEntry {
            title: if t.is_empty() { u.to_string() } else { t.to_string() },
            url: u.to_string(),
            visited_epoch,
        });
    }
    list.truncate(MAX_HISTORY_ENTRIES);
    list
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
    /// Başlangıç adresi ve pencere genişliği ile ilk sekmeyi ve diskteki kalıcı verileri yükler.
    pub fn new(initial_address: &str, viewport_width: u32) -> Self {
        let (bookmarks_path, cookies_path, history_path) = if cfg!(test) {
            (None, None, None)
        } else {
            (
                default_bookmarks_path(),
                default_cookies_path(),
                default_history_path(),
            )
        };
        Self::with_all_storage_paths(
            initial_address,
            viewport_width,
            bookmarks_path,
            cookies_path,
            history_path,
        )
    }

    /// Belirli bir yer imi dosya yoluyla (gerçek disk veya test geçici dosyası) `BrowserState` başlatır.
    #[cfg(test)]
    pub fn with_bookmarks_path(
        initial_address: &str,
        viewport_width: u32,
        bookmarks_file: Option<PathBuf>,
    ) -> Self {
        Self::with_all_storage_paths(initial_address, viewport_width, bookmarks_file, None, None)
    }

    /// Yer imi ve kalıcı çerez dosya yollarıyla (test uyumluluğu için) `BrowserState` başlatır.
    #[cfg(test)]
    pub fn with_storage_paths(
        initial_address: &str,
        viewport_width: u32,
        bookmarks_file: Option<PathBuf>,
        cookies_file: Option<PathBuf>,
    ) -> Self {
        Self::with_all_storage_paths(
            initial_address,
            viewport_width,
            bookmarks_file,
            cookies_file,
            None,
        )
    }

    /// Yer imi, kalıcı çerez ve geçmiş dosya yollarıyla `BrowserState` başlatır.
    pub fn with_all_storage_paths(
        initial_address: &str,
        viewport_width: u32,
        bookmarks_file: Option<PathBuf>,
        cookies_file: Option<PathBuf>,
        history_file: Option<PathBuf>,
    ) -> Self {
        let bookmarks = bookmarks_file
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|raw| deserialize_bookmarks(&raw))
            .filter(|list| !list.is_empty())
            .unwrap_or_else(default_bookmarks);

        let mut cookies = cookies_file
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|raw| CookieJar::deserialize_tsv(&raw))
            .unwrap_or_default();
        cookies.prune_expired();

        let history_entries = history_file
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|raw| deserialize_history(&raw))
            .unwrap_or_default();

        let mut state = Self {
            tabs: vec![TabState {
                current_url: String::new(),
                page: RenderSnapshot::empty(),
                worker: TabWorkerClient::new(!cfg!(test)),
                history: Vec::new(),
                forward_history: Vec::new(),
                scroll_y: 0,
                focused_input: None,
                input_values: HashMap::new(),
                zoom_percent: 100,
                find_open: false,
                find_query: String::new(),
                find_matches: Vec::new(),
                find_active: 0,
            }],
            active_tab: 0,
            bookmarks,
            history_entries,
            viewport_width,
            shield_enabled: true,
            cookies,
            bookmarks_file,
            cookies_file,
            history_file,
        };
        state.persist_cookies();
        state.open_request(initial_address, None, false);
        state
    }

    fn write_config_file(path_opt: &Option<PathBuf>, content: &str) {
        if let Some(path) = path_opt {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, content);
        }
    }

    /// Güncel yer imlerini diskteki `bookmarks.tsv` dosyasına kalıcı olarak yazar.
    fn persist_bookmarks(&self) {
        let content = serialize_bookmarks(&self.bookmarks);
        Self::write_config_file(&self.bookmarks_file, &content);
    }

    /// Süresi dolmamış kalıcı çerezleri diskteki `cookies.tsv` dosyasına yazar.
    fn persist_cookies(&self) {
        let content = self.cookies.serialize_tsv();
        Self::write_config_file(&self.cookies_file, &content);
    }

    /// Ziyaret geçmişini diskteki `history.tsv` dosyasına kalıcı olarak yazar.
    fn persist_history(&self) {
        let content = serialize_history(&self.history_entries);
        Self::write_config_file(&self.history_file, &content);
    }

    fn record_history_visit(&mut self, url: &str, title: Option<&str>) {
        if url.is_empty()
            || url.starts_with("view-source:")
            || url.starts_with("http://anka.local/gecmis")
            || url == "http://anka.local/cerez-temizle"
            || url == "http://anka.local/cokme-testi"
        {
            return;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let entry_title = title
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .unwrap_or(url)
            .to_string();
        self.history_entries.retain(|h| h.url != url);
        self.history_entries.insert(
            0,
            HistoryEntry {
                title: entry_title,
                url: url.to_string(),
                visited_epoch: now,
            },
        );
        self.history_entries.truncate(MAX_HISTORY_ENTRIES);
        self.persist_history();
    }

    /// Tüm gezinme geçmişini bellekten ve diskten temizler.
    pub fn clear_history(&mut self) {
        self.history_entries.clear();
        self.persist_history();
    }

    /// Tüm çerezleri bellekten ve diskten temizler.
    pub fn clear_cookies(&mut self) {
        self.cookies.clear();
        self.persist_cookies();
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

    pub fn focused_is_multiline(&self) -> bool {
        let Some(focused) = self.focused_input() else {
            return false;
        };
        self.active()
            .page
            .display_list
            .iter()
            .any(|k| k.input_name.as_deref() == Some(focused) && k.is_multiline)
    }

    /// Çok satırlı `<textarea>` metnini `\n` ve maksimum satır genişliğine göre satırlara böler.
    pub fn wrap_display_lines(text: &str, max_chars: usize) -> Vec<String> {
        let cap = max_chars.max(8);
        let mut lines = Vec::new();
        for raw_line in text.split('\n') {
            if raw_line.is_empty() {
                lines.push(String::new());
                continue;
            }
            let mut cur = String::new();
            let mut count = 0usize;
            for ch in raw_line.chars() {
                if count >= cap {
                    lines.push(cur);
                    cur = String::new();
                    count = 0;
                }
                cur.push(ch);
                count += 1;
            }
            lines.push(cur);
        }
        lines
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
            "[Sekme {}/{} | {} | %{} | {}] {}",
            self.active_tab + 1,
            self.tabs.len(),
            pid_tag,
            active.zoom_percent,
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
            Some(t) => format!("{t} {tab_info} — Anka Browser v0.3.4"),
            None => format!("Anka Browser v0.3.4 {tab_info} — Telek Engine"),
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

    fn record_snapshot_cookies(&mut self, request_url: &str, set_cookies: &[String]) {
        if !set_cookies.is_empty()
            && let Ok(parsed) = Url::parse(request_url)
        {
            for line in set_cookies {
                self.cookies.store_header_line(&parsed, line);
            }
            self.persist_cookies();
        }
    }

    fn cookie_header_for_str(&self, raw_url: &str) -> Option<String> {
        let norm = normalize_address(raw_url);
        let stripped = norm.strip_prefix("view-source:").unwrap_or(&norm);
        Url::parse(stripped)
            .ok()
            .and_then(|u| self.cookies.cookie_header_for(&u))
    }

    pub fn new_tab(&mut self, initial_address: &str) -> usize {
        self.tabs.push(TabState {
            current_url: String::new(),
            page: RenderSnapshot::empty(),
            worker: TabWorkerClient::new(!cfg!(test)),
            history: Vec::new(),
            forward_history: Vec::new(),
            scroll_y: 0,
            focused_input: None,
            input_values: HashMap::new(),
            zoom_percent: 100,
            find_open: false,
            find_query: String::new(),
            find_matches: Vec::new(),
            find_active: 0,
        });
        self.active_tab = self.tabs.len() - 1;
        self.open_request(initial_address, None, false);
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

    fn build_history_http_response(&self, filter_query: &str) -> String {
        let needle = filter_query.trim().to_lowercase();
        let mut items_html = String::new();
        let mut matched_count = 0usize;

        for entry in &self.history_entries {
            if !needle.is_empty()
                && !entry.title.to_lowercase().contains(&needle)
                && !entry.url.to_lowercase().contains(&needle)
            {
                continue;
            }
            matched_count += 1;
            let safe_title = html_escape(&entry.title);
            let safe_url = html_escape(&entry.url);
            let when = format_epoch_utc(entry.visited_epoch);
            items_html.push_str(&format!(
                "<div class=\"kart\"><p>{matched_count}. {safe_title} ({when})</p><a class=\"pill\" href=\"{safe_url}\">{safe_url}</a></div>"
            ));
        }

        if items_html.is_empty() {
            items_html = if needle.is_empty() {
                "<div id=\"duyuru\"><p>Henuz hic gezinme gecmisi kaydi yok.</p></div>".to_string()
            } else {
                format!(
                    "<div id=\"duyuru\"><p>\"{}\" aramasiyla eslesen kayit bulunamadi.</p></div>",
                    html_escape(filter_query)
                )
            };
        }

        let safe_filter = html_escape(filter_query);
        format!(
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n",
                "<html><head><title>Gezinme Gecmisi</title><style>",
                "h1 {{ color: #1e3a8a; background-color: #eff6ff; border: 1px solid #bfdbfe; border-radius: 8px; padding: 8px; font-size: 26px; }}",
                "p {{ color: #1e293b; font-size: 15px; }}",
                ".pill {{ color: #1e40af; background-color: #dbeafe; border: 1px solid #93c5fd; border-radius: 6px; font-size: 14px; }}",
                ".cikis {{ color: #ffffff; background-color: #dc2626; border: 1px solid #b91c1c; border-radius: 6px; font-size: 14px; }}",
                ".kart {{ color: #065f46; background-color: #ecfdf5; border: 1px solid #6ee7b7; border-radius: 8px; padding: 8px; font-size: 15px; }}",
                "#duyuru {{ color: #9a3412; background-color: #ffedd5; border: 1px solid #fdba74; border-radius: 8px; padding: 8px; font-size: 15px; }}",
                "</style></head><body>",
                "<h1>Anka Tarayici Gecmisi ({} Kayit)</h1>",
                "<nav>",
                "<a class=\"pill\" href=\"http://anka.local/hosgeldiniz\">Ana Sayfa</a>",
                "<a class=\"pill\" href=\"http://anka.local/gecmis\">Tum Gecmis</a>",
                "<a class=\"cikis\" href=\"http://anka.local/gecmis-temizle\">Gecmisi Temizle</a>",
                "</nav>",
                "<form action=\"http://anka.local/gecmis\">",
                "<input type=\"text\" name=\"q\" value=\"{}\" placeholder=\"Gecmiste baslik veya adres ara...\">",
                "<input type=\"submit\" value=\"Gecmiste Ara\">",
                "</form>",
                "{}",
                "</body></html>"
            ),
            matched_count, safe_filter, items_html
        )
    }

    fn build_devtools_http_response(&self, inspected_url: &str, snap: &RenderSnapshot) -> String {
        let mut cookie_html = String::new();
        for c in self.cookies.all() {
            let lifetime = if c.expires_epoch.is_some() {
                "Kalici (Disk)"
            } else {
                "Oturum"
            };
            let mut flags = String::new();
            if c.secure {
                flags.push_str(" Secure");
            }
            if c.http_only {
                flags.push_str(" HttpOnly");
            }
            cookie_html.push_str(&format!(
                "<div class=\"kart\"><p>{} = {}</p><p>Alan: {}{} | Tur: {}{}</p></div>",
                html_escape(&c.name),
                html_escape(&c.value),
                html_escape(&c.domain),
                html_escape(&c.path),
                lifetime,
                flags
            ));
        }
        if cookie_html.is_empty() {
            cookie_html =
                "<div id=\"duyuru\"><p>CookieJar su an bos. Oturum Testi sayfasindan POST ile giris yaparak kalici cerez olusturabilirsiniz.</p></div>"
                    .to_string();
        }

        let mut logs_html = String::new();
        for (i, line) in snap.js_logs.iter().enumerate() {
            logs_html.push_str(&format!("<li>[{}] {}</li>", i + 1, html_escape(line)));
        }
        if logs_html.is_empty() {
            logs_html = "<li>(Bu sayfada henuz JS console.log kaydi yok)</li>".to_string();
        }

        let safe_url = html_escape(inspected_url);
        let source_target = if inspected_url.starts_with("view-source:") {
            inspected_url.to_string()
        } else {
            format!("view-source:{inspected_url}")
        };
        let safe_source_url = html_escape(&source_target);

        format!(
            concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n",
                "<html><head><title>Gelistirici Araclari (DevTools)</title><style>",
                "h1 {{ color: #1e293b; background-color: #f1f5f9; border: 1px solid #cbd5e1; border-radius: 8px; padding: 8px; font-size: 26px; }}",
                "h2 {{ color: #1d4ed8; font-size: 19px; }}",
                "p, li {{ color: #334155; font-size: 15px; }}",
                ".pill {{ color: #1e40af; background-color: #dbeafe; border: 1px solid #93c5fd; border-radius: 6px; font-size: 14px; }}",
                ".cikis {{ color: #ffffff; background-color: #dc2626; border: 1px solid #b91c1c; border-radius: 6px; font-size: 14px; }}",
                ".kart {{ color: #065f46; background-color: #ecfdf5; border: 1px solid #6ee7b7; border-radius: 8px; padding: 8px; font-size: 15px; }}",
                "#duyuru {{ color: #9a3412; background-color: #ffedd5; border: 1px solid #fdba74; border-radius: 8px; padding: 8px; font-size: 15px; }}",
                ".konsol {{ color: #e2e8f0; background-color: #0f172a; border: 1px solid #334155; border-radius: 8px; padding: 10px; font-size: 14px; }}",
                "</style></head><body>",
                "<h1>Anka DevTools — Sekme &amp; Ag Mufettisi</h1>",
                "<nav>",
                "<a class=\"pill\" href=\"http://anka.local/hosgeldiniz\">Ana Sayfa</a>",
                "<a class=\"pill\" href=\"{}\">Sayfa Kaynagini Gor (Ctrl+U)</a>",
                "<a class=\"cikis\" href=\"http://anka.local/cerez-temizle\">Cerezleri Temizle</a>",
                "</nav>",
                "<div class=\"kart\"><p>Incelenen Adres: {} | Isci PID: {} | Anka Kalkan Engeli: {}</p></div>",
                "<h2>1. JavaScript Konsol Ciktisi ({} Kayit)</h2>",
                "<div class=\"konsol\"><ul>{}</ul></div>",
                "<h2>2. Cookie Jar ({} Aktif Cerez)</h2>",
                "{}",
                "</body></html>"
            ),
            safe_source_url,
            safe_url,
            snap.pid,
            snap.shield_blocked,
            snap.js_logs.len(),
            logs_html,
            self.cookies.count(),
            cookie_html
        )
    }

    fn open_request(&mut self, raw_address: &str, post_body: Option<&str>, push_history: bool) {
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
            tab.history.push(current_url_before.clone());
            tab.forward_history.clear();
        }

        if clean_url == "http://anka.local/cerez-temizle" {
            self.clear_cookies();
        } else if clean_url == "http://anka.local/gecmis-temizle" {
            self.clear_history();
        }

        let viewport_w = self.viewport_width;
        let shield_on = self.shield_enabled;
        let cookie_hdr = self.cookie_header_for_str(&clean_url);
        let loaded = if let Some(body) = post_body {
            self.active_mut().worker.post_url(
                raw_address,
                body,
                viewport_w,
                shield_on,
                cookie_hdr.as_deref(),
            )
        } else if clean_url == "http://anka.local/yer-imleri" {
            let raw_http = self.build_bookmarks_http_response();
            self.active_mut()
                .worker
                .load_html(&clean_url, &raw_http, viewport_w, shield_on)
        } else if clean_url == "http://anka.local/gecmis"
            || clean_url.starts_with("http://anka.local/gecmis?")
        {
            let query_str = clean_url.split_once('?').map(|(_, q)| q).unwrap_or("");
            let filter_q = extract_form_field(query_str, "q");
            let raw_http = self.build_history_http_response(&filter_q);
            self.active_mut()
                .worker
                .load_html(&clean_url, &raw_http, viewport_w, shield_on)
        } else if clean_url == "http://anka.local/gecmis-temizle" {
            let raw_http = concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n",
                "<html><head><title>Gecmis Temizlendi</title><style>",
                "h1 { color: #065f46; background-color: #ecfdf5; border: 1px solid #6ee7b7; border-radius: 8px; padding: 8px; }",
                ".pill { color: #1e40af; background-color: #dbeafe; border: 1px solid #93c5fd; border-radius: 6px; font-size: 15px; }",
                "</style></head><body>",
                "<h1>Tum Gezinme Gecmisi Temizlendi!</h1>",
                "<p>history.tsv dosyasi sifirlandi.</p>",
                "<nav><a class=\"pill\" href=\"http://anka.local/gecmis\">Gecmis Sayfasina Don</a>",
                "<a class=\"pill\" href=\"http://anka.local/hosgeldiniz\">Ana Sayfa</a></nav>",
                "</body></html>"
            );
            self.active_mut()
                .worker
                .load_html(&clean_url, raw_http, viewport_w, shield_on)
        } else if clean_url == "http://anka.local/gelistirici" {
            let inspected_url = if current_url_before.is_empty() {
                clean_url.clone()
            } else {
                current_url_before
            };
            let raw_http = self.build_devtools_http_response(&inspected_url, &self.active().page);
            self.active_mut()
                .worker
                .load_html(&clean_url, &raw_http, viewport_w, shield_on)
        } else if clean_url == "http://anka.local/cerez-temizle" {
            let raw_http = concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n",
                "<html><head><title>Cerezler Temizlendi</title><style>",
                "h1 { color: #065f46; background-color: #ecfdf5; border: 1px solid #6ee7b7; border-radius: 8px; padding: 8px; }",
                ".pill { color: #1e40af; background-color: #dbeafe; border: 1px solid #93c5fd; border-radius: 6px; font-size: 15px; }",
                "</style></head><body>",
                "<h1>Tum Cerezler ve Kalici Oturumlar Temizlendi!</h1>",
                "<p>CookieJar ve cookies.tsv sifirlandi.</p>",
                "<nav><a class=\"pill\" href=\"http://anka.local/gelistirici\">Gelistirici Paneline Don</a>",
                "<a class=\"pill\" href=\"http://anka.local/hosgeldiniz\">Ana Sayfa</a></nav>",
                "</body></html>"
            );
            self.active_mut()
                .worker
                .load_html(&clean_url, raw_http, viewport_w, shield_on)
        } else {
            self.active_mut().worker.load_url(
                raw_address,
                viewport_w,
                shield_on,
                cookie_hdr.as_deref(),
            )
        };

        self.record_snapshot_cookies(&clean_url, &loaded.set_cookies);
        let loaded_title = loaded.page_title.clone();
        let loaded_crashed = loaded.crashed;

        {
            let tab = self.active_mut();
            tab.current_url = clean_url.clone();
            tab.page = loaded;
            tab.scroll_y = 0;
            tab.focused_input = None;
            tab.input_values.clear();
            tab.find_open = false;
            tab.find_query.clear();
            tab.find_matches.clear();
            tab.find_active = 0;
        }

        if !loaded_crashed {
            self.record_history_visit(&clean_url, loaded_title.as_deref());
        }
    }

    pub fn navigate(&mut self, raw_address: &str, push_history: bool) {
        self.open_request(raw_address, None, push_history);
    }

    pub fn submit_post(&mut self, raw_address: &str, body: &str, push_history: bool) {
        self.open_request(raw_address, Some(body), push_history);
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

    /// Aktif sekmenin yakınlaştırma oranını (%) döndürür (`50..=200`).
    pub fn zoom_percent(&self) -> u32 {
        self.active().zoom_percent
    }

    /// Aktif sekmeyi bir kademe yakınlaştırır (`Ctrl+` / `Ctrl+=`).
    pub fn zoom_in(&mut self) -> u32 {
        let tab = self.active_mut();
        if let Some(&next) = ZOOM_LEVELS.iter().find(|&&z| z > tab.zoom_percent) {
            tab.zoom_percent = next;
        }
        tab.zoom_percent
    }

    /// Aktif sekmeyi bir kademe uzaklaştırır (`Ctrl-`).
    pub fn zoom_out(&mut self) -> u32 {
        let tab = self.active_mut();
        if let Some(&prev) = ZOOM_LEVELS.iter().rfind(|&&z| z < tab.zoom_percent) {
            tab.zoom_percent = prev;
        }
        tab.zoom_percent
    }

    /// Aktif sekmenin yakınlaştırmasını `%100` varsayılan değerine sıfırlar (`Ctrl+0`).
    pub fn zoom_reset(&mut self) -> u32 {
        let tab = self.active_mut();
        tab.zoom_percent = 100;
        100
    }

    /// Yakınlaştırılmış/uzaklaştırılmış tuval koordinatını ham sayfa (layout) koordinatına çevirir.
    fn canvas_to_layout(&self, canvas_x: i32, canvas_y: i32) -> (i32, i32) {
        let zoom = self.active().zoom_percent.max(50) as i32;
        ((canvas_x * 100) / zoom, (canvas_y * 100) / zoom)
    }

    /// Sayfa içi arama (`Ctrl+F`) çubuğu açık mı?
    pub fn is_find_open(&self) -> bool {
        self.active().find_open
    }

    /// Sayfa içi arama (`Ctrl+F`) sorgu metnini döndürür.
    pub fn find_query(&self) -> &str {
        &self.active().find_query
    }

    /// Arama çubuğunun sağında gösterilen `1/4` veya `0/0 eslesme` durum metnini üretir.
    pub fn find_status_text(&self) -> String {
        let tab = self.active();
        if tab.find_query.is_empty() {
            "Aramak icin yazin".to_string()
        } else if tab.find_matches.is_empty() {
            "0/0 eslesme".to_string()
        } else {
            format!("{}/{}", tab.find_active + 1, tab.find_matches.len())
        }
    }

    /// Belirtilen `display_list` komutunun aktif arama sonucu (`Some(true)`), diğer eşleşme
    /// (`Some(false)`) veya eşleşmeyen (`None`) olduğunu döndürür.
    pub fn find_match_kind(&self, cmd_idx: usize) -> Option<bool> {
        let tab = self.active();
        if !tab.find_open || tab.find_matches.is_empty() {
            return None;
        }
        if tab.find_matches.get(tab.find_active) == Some(&cmd_idx) {
            Some(true)
        } else if tab.find_matches.contains(&cmd_idx) {
            Some(false)
        } else {
            None
        }
    }

    /// `Ctrl+F` sayfa içi arama çubuğunu açar ve mevcut sorgu varsa eşleşmeleri günceller.
    pub fn open_find(&mut self) {
        self.active_mut().find_open = true;
        if !self.active().find_query.is_empty() {
            self.recompute_find_matches();
        }
    }

    /// Sayfa içi arama çubuğunu (`Esc` / `Kapat`) kapatır ve vurguları temizler.
    pub fn close_find(&mut self) {
        let tab = self.active_mut();
        tab.find_open = false;
        tab.find_matches.clear();
        tab.find_active = 0;
    }

    /// Sayfa içi arama sorgusunu günceller, büyük/küçük harf duyarsız eşleşmeleri hesaplar
    /// ve ilk eşleşmeye otomatik kaydırır.
    pub fn set_find_query(&mut self, query: &str) {
        {
            let tab = self.active_mut();
            tab.find_open = true;
            if tab.find_query == query && (!query.is_empty() && !tab.find_matches.is_empty()) {
                return;
            }
            tab.find_query = query.to_string();
        }
        self.recompute_find_matches();
    }

    /// Sonraki (`reverse = false`, `Enter`) veya önceki (`reverse = true`, `Shift+Enter`)
    /// eşleşmeye atlar ve görünümü o kutuya kaydırır.
    pub fn find_next(&mut self, reverse: bool) -> bool {
        {
            let tab = self.active_mut();
            if tab.find_matches.is_empty() {
                return false;
            }
            let len = tab.find_matches.len();
            tab.find_active = if reverse {
                (tab.find_active + len - 1) % len
            } else {
                (tab.find_active + 1) % len
            };
        }
        self.scroll_to_active_find_match();
        true
    }

    fn recompute_find_matches(&mut self) {
        let needle = self.active().find_query.trim().to_lowercase();
        if needle.is_empty() {
            let tab = self.active_mut();
            tab.find_matches.clear();
            tab.find_active = 0;
            return;
        }
        let matches: Vec<usize> = self
            .active()
            .page
            .display_list
            .iter()
            .enumerate()
            .filter_map(|(idx, cmd)| {
                if !cmd.is_password && cmd.text.to_lowercase().contains(&needle) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        {
            let tab = self.active_mut();
            tab.find_matches = matches;
            tab.find_active = 0;
        }
        self.scroll_to_active_find_match();
    }

    fn scroll_to_active_find_match(&mut self) {
        let tab = self.active_mut();
        let Some(&cmd_idx) = tab.find_matches.get(tab.find_active) else {
            return;
        };
        let Some(cmd) = tab.page.display_list.get(cmd_idx) else {
            return;
        };
        let match_y = cmd.y as i32;
        let visible_h = (480 * 100) / tab.zoom_percent.max(50) as i32;
        if match_y < tab.scroll_y || match_y + 24 > tab.scroll_y + visible_h {
            tab.scroll_y = (match_y - visible_h / 4).max(0);
        }
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

    /// Klavyeden `Tab` / `Shift+Tab` basıldığında sayfadaki `<input>` kutuları arasında odak geçişi yapar.
    pub fn focus_next_input(&mut self, reverse: bool) -> bool {
        let names: Vec<String> = self
            .active()
            .page
            .display_list
            .iter()
            .filter_map(|k| k.input_name.clone())
            .collect();
        if names.is_empty() {
            return false;
        }

        let next_idx = match self
            .active()
            .focused_input
            .as_deref()
            .and_then(|cur| names.iter().position(|n| n == cur))
        {
            Some(cur_idx) => {
                if reverse {
                    if cur_idx == 0 {
                        names.len() - 1
                    } else {
                        cur_idx - 1
                    }
                } else {
                    (cur_idx + 1) % names.len()
                }
            }
            None => {
                if reverse {
                    names.len() - 1
                } else {
                    0
                }
            }
        };

        self.active_mut().focused_input = Some(names[next_idx].clone());
        true
    }

    /// Odaktaki `<input>` seçimini temizler (`Escape`).
    pub fn blur_input(&mut self) -> bool {
        self.active_mut().focused_input.take().is_some()
    }

    fn collect_form_params(&self, action: &str) -> Vec<String> {
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
        params
    }

    fn resolve_form_target(&self, action: &str) -> Option<Url> {
        let tab = self.active();
        let target_raw = if action.is_empty() {
            tab.current_url.as_str()
        } else {
            action
        };
        let base_url = Url::parse(&tab.current_url).ok()?;
        base_url.resolve(target_raw).ok()
    }

    /// Formu `GET` (`Navigate`) veya `POST` (`SubmitPost`) olarak göndermek için tam eylemi oluşturur.
    pub fn build_form_submit_action(&self, action: &str, is_post: bool) -> Option<ClickAction> {
        let resolved = self.resolve_form_target(action)?;
        let port_part = resolved
            .port
            .map(|p| format!(":{p}"))
            .unwrap_or_default();
        let query_joined = self.collect_form_params(action).join("&");

        if is_post {
            let query_part = resolved
                .query
                .as_ref()
                .map(|q| format!("?{q}"))
                .unwrap_or_default();
            return Some(ClickAction::SubmitPost {
                url: format!(
                    "{}://{}{}{}{}",
                    resolved.scheme, resolved.host, port_part, resolved.path, query_part
                ),
                body: query_joined,
            });
        }

        let full_query = match (&resolved.query, query_joined.is_empty()) {
            (Some(existing), false) => format!("?{existing}&{query_joined}"),
            (Some(existing), true) => format!("?{existing}"),
            (None, false) => format!("?{query_joined}"),
            (None, true) => String::new(),
        };

        Some(ClickAction::Navigate(format!(
            "{}://{}{}{}{}",
            resolved.scheme, resolved.host, port_part, resolved.path, full_query
        )))
    }

    /// Odaklı `<input>` kutusunda `Enter` tuşuna basıldığında bağlı olduğu formu (`GET` veya `POST`) gönderir!
    pub fn submit_focused_form(&self) -> ClickAction {
        let Some(focused) = self.active().focused_input.as_deref() else {
            return ClickAction::None;
        };
        let Some(cmd) = self
            .active()
            .page
            .display_list
            .iter()
            .find(|k| k.input_name.as_deref() == Some(focused))
        else {
            return ClickAction::None;
        };
        let Some(action) = cmd.form_action.as_deref() else {
            return ClickAction::None;
        };
        self.build_form_submit_action(action, cmd.is_post_form)
            .unwrap_or(ClickAction::None)
    }

    #[allow(dead_code)]
    pub fn is_interactive_at(&self, canvas_x: i32, canvas_y: i32) -> bool {
        let (lx, ly) = self.canvas_to_layout(canvas_x, canvas_y);
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
                lx >= draw_x && lx <= draw_right && ly >= draw_y && ly <= draw_bottom
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
        let (lx, ly) = self.canvas_to_layout(canvas_x, canvas_y);
        let tab = self.active();
        let mut found_input: Option<String> = None;
        let mut found_submit: Option<(String, bool)> = None;
        let mut found_onclick: Option<String> = None;
        let mut found_href: Option<String> = None;

        for komut in &tab.page.display_list {
            let draw_x = komut.x as i32 + 4;
            let draw_right = draw_x + komut.width as i32;
            let draw_y = komut.y as i32 - tab.scroll_y;
            let draw_bottom = draw_y + komut.height as i32;

            if lx >= draw_x && lx <= draw_right && ly >= draw_y && ly <= draw_bottom {
                if let Some(name) = &komut.input_name {
                    found_input = Some(name.clone());
                    break;
                }
                if let Some(js_code) = &komut.onclick {
                    found_onclick = Some(js_code.clone());
                    break;
                }
                if komut.is_submit {
                    found_submit = Some((
                        komut.form_action.clone().unwrap_or_default(),
                        komut.is_post_form,
                    ));
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
            {
                let tab = self.active_mut();
                tab.page = tab.worker.eval_js(&js_code, vw);
            }
            if self.active().find_open && !self.active().find_query.is_empty() {
                self.recompute_find_matches();
            }
            return ClickAction::Redraw;
        }

        if let Some((action, is_post)) = found_submit
            && let Some(submit_action) = self.build_form_submit_action(&action, is_post)
        {
            return submit_action;
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
        let submit_action = state.submit_focused_form();
        assert_eq!(
            submit_action,
            ClickAction::Navigate("https://tr.wikipedia.org/w/index.php?search=Anka+kusu".to_string())
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

    #[test]
    fn test_oturum_post_ve_sekmeler_arasi_cerez_paylasimi() {
        let mut state = BrowserState::new("http://anka.local/oturum", 860);
        assert_eq!(state.cookies.count(), 0);

        let user_box = state
            .display_list()
            .iter()
            .find(|b| b.input_name.as_deref() == Some("kullanici"))
            .expect("kullanici kutusu")
            .clone();
        let pw_box = state
            .display_list()
            .iter()
            .find(|b| b.input_name.as_deref() == Some("sifre"))
            .expect("sifre kutusu")
            .clone();
        assert!(pw_box.is_password);
        assert!(pw_box.is_post_form);

        assert_eq!(
            state.click_at(user_box.x as i32 + 10, user_box.y as i32 + 5),
            ClickAction::Redraw
        );
        for ch in "ekrem".chars() {
            assert!(state.type_char(ch));
        }

        assert_eq!(
            state.click_at(pw_box.x as i32 + 10, pw_box.y as i32 + 5),
            ClickAction::Redraw
        );
        for ch in "gizli99".chars() {
            assert!(state.type_char(ch));
        }

        let submit_action = state.submit_focused_form();
        assert_eq!(
            submit_action,
            ClickAction::SubmitPost {
                url: "http://anka.local/oturum".to_string(),
                body: "kullanici=ekrem&sifre=gizli99".to_string(),
            }
        );

        if let ClickAction::SubmitPost { url, body } = submit_action {
            state.submit_post(&url, &body, true);
        }

        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("Hos geldiniz, ekrem!"))
        );
        assert!(state.cookies.count() >= 2);

        // Yeni bir sekme acinca Shared CookieJar sayesinde otomatik girisli gorunmeli!
        state.new_tab("http://anka.local/oturum");
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("Hos geldiniz, ekrem!"))
        );

        // Oturumu kapatinca (Max-Age=0) tekrar form gorunmeli:
        state.navigate("http://anka.local/oturum-cikis", true);
        state.navigate("http://anka.local/oturum", true);
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.input_name.as_deref() == Some("kullanici"))
        );
    }

    #[test]
    fn test_kalici_cerez_disk_kaydi_ve_yeniden_acilis() {
        let cookies_path = std::env::temp_dir().join("anka_test_cookies_v032.tsv");
        let _ = std::fs::remove_file(&cookies_path);

        {
            let mut state = BrowserState::with_storage_paths(
                "http://anka.local/oturum",
                860,
                None,
                Some(cookies_path.clone()),
            );
            state.submit_post("http://anka.local/oturum", "kullanici=demir&sifre=pw99", true);
            assert!(
                state
                    .display_list()
                    .iter()
                    .any(|b| b.text.contains("Hos geldiniz, demir!"))
            );
        }

        // Tarayici tamamen kapatilip ayni cookies.tsv ile yeniden acildiginda oturum hatirlanmali!
        let mut reopened = BrowserState::with_storage_paths(
            "http://anka.local/oturum",
            860,
            None,
            Some(cookies_path.clone()),
        );
        assert!(
            reopened
                .display_list()
                .iter()
                .any(|b| b.text.contains("Hos geldiniz, demir!"))
        );

        // Cerezleri temizle rotasi hem bellegi hem diski sifirlamali:
        reopened.navigate("http://anka.local/cerez-temizle", true);
        assert_eq!(reopened.cookies.count(), 0);
        let _ = std::fs::remove_file(&cookies_path);
    }

    #[test]
    fn test_devtools_view_source_ve_tab_odak_gecisi() {
        let mut state = BrowserState::new("http://anka.local/oturum", 860);
        assert_eq!(state.focused_input(), None);

        // Tab -> ilk alan (`kullanici`), tekrar Tab -> ikinci alan (`sifre`), Shift+Tab -> `kullanici`, Esc -> None:
        assert!(state.focus_next_input(false));
        assert_eq!(state.focused_input(), Some("kullanici"));
        assert!(state.focus_next_input(false));
        assert_eq!(state.focused_input(), Some("sifre"));
        assert!(state.focus_next_input(true));
        assert_eq!(state.focused_input(), Some("kullanici"));
        assert!(state.blur_input());
        assert_eq!(state.focused_input(), None);

        // Ana sayfadaki JS konsol loglari DevTools'da gorunmeli:
        state.navigate("http://anka.local/hosgeldiniz", true);
        state.navigate("http://anka.local/gelistirici", true);
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("Anka JS v0.3.4 baslatildi"))
        );

        // view-source: sayfanin ham HTML kaynağını satır numaralarıyla göstermeli:
        state.navigate("view-source:http://anka.local/hosgeldiniz", true);
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("Sayfa Kaynagi"))
        );
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("<html>"))
        );
    }

    #[test]
    fn test_kalici_gecmis_kaydi_arama_ve_temizleme() {
        let history_path = std::env::temp_dir().join("anka_test_history_v033.tsv");
        let _ = std::fs::remove_file(&history_path);

        {
            let mut state = BrowserState::with_all_storage_paths(
                "http://anka.local/hosgeldiniz",
                860,
                None,
                None,
                Some(history_path.clone()),
            );
            state.navigate("http://anka.local/oturum", true);
            state.navigate("http://anka.local/kalkan", true);
            assert!(state.history_entries.len() >= 3);
            assert_eq!(state.history_entries[0].url, "http://anka.local/kalkan");
        }

        let mut reopened = BrowserState::with_all_storage_paths(
            "http://anka.local/gecmis?q=Kalkan",
            860,
            None,
            None,
            Some(history_path.clone()),
        );
        assert!(
            reopened
                .display_list()
                .iter()
                .any(|b| b.text.contains("Anka Kalkan Testi"))
        );
        assert!(
            !reopened
                .display_list()
                .iter()
                .any(|b| b.text.contains("Oturum Testi"))
        );

        reopened.navigate("http://anka.local/gecmis-temizle", true);
        assert!(reopened.history_entries.is_empty());
        let _ = std::fs::remove_file(&history_path);
    }

    #[test]
    fn test_textarea_satir_sonu_ve_addeventlistener() {
        let mut state = BrowserState::new("http://anka.local/hosgeldiniz", 860);

        // Ana sayfadaki `Sifirla` dugmesi querySelector + addEventListener ile baglanmis olmali!
        let artir_btn = state
            .display_list()
            .iter()
            .find(|b| b.onclick.as_deref() == Some("artir()"))
            .unwrap()
            .clone();
        assert_eq!(
            state.click_at(artir_btn.x as i32 + 5, artir_btn.y as i32 + 5),
            ClickAction::Redraw
        );
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("Tiklama Sayisi: 1"))
        );

        let sifirla_btn = state
            .display_list()
            .iter()
            .find(|b| b.text.contains("Sifirla"))
            .expect("Sifirla dugmesi")
            .clone();
        assert_eq!(sifirla_btn.onclick.as_deref(), Some("sifirla()"));
        assert_eq!(
            state.click_at(sifirla_btn.x as i32 + 5, sifirla_btn.y as i32 + 5),
            ClickAction::Redraw
        );
        assert!(
            state
                .display_list()
                .iter()
                .any(|b| b.text.contains("Sayac Sifirlandi: 0"))
        );

        // Not Defteri (`<textarea>` + POST):
        state.navigate("http://anka.local/not", true);
        let area = state
            .display_list()
            .iter()
            .find(|b| b.tag == "textarea")
            .expect("textarea kutusu")
            .clone();
        assert!(area.is_multiline);
        assert_eq!(
            state.click_at(area.x as i32 + 8, area.y as i32 + 8),
            ClickAction::Redraw
        );
        assert!(state.focused_is_multiline());
        for ch in "Ilk\nIkinci".chars() {
            assert!(state.type_char(ch));
        }

        let lines = BrowserState::wrap_display_lines(state.input_value("icerik").unwrap(), 30);
        assert_eq!(lines, vec!["Ilk".to_string(), "Ikinci".to_string()]);

        let submit_btn = state
            .display_list()
            .iter()
            .find(|b| b.is_submit)
            .expect("Notu Gonder dugmesi")
            .clone();
        let action = state.click_at(submit_btn.x as i32 + 8, submit_btn.y as i32 + 5);
        assert!(matches!(action, ClickAction::SubmitPost { .. }));
        if let ClickAction::SubmitPost { url, body } = action {
            state.submit_post(&url, &body, true);
        }
        assert!(state.display_list().iter().any(|b| b.text == "Ilk"));
        assert!(state.display_list().iter().any(|b| b.text == "Ikinci"));
    }

    #[test]
    fn test_zoom_olcekleme_ve_koordinat_donusumu() {
        let mut state = BrowserState::new("http://anka.local/hosgeldiniz", 860);
        assert_eq!(state.zoom_percent(), 100);
        assert!(state.status_message().contains("%100"));

        assert_eq!(state.zoom_in(), 110);
        assert_eq!(state.zoom_in(), 125);
        assert!(state.status_message().contains("%125"));

        // Her sekme kendi yakınlaştırma seviyesini bağımsız saklamalı:
        state.new_tab("http://anka.local/oturum");
        assert_eq!(state.zoom_percent(), 100);
        assert!(state.switch_tab(0));
        assert_eq!(state.zoom_percent(), 125);

        // %125 büyütülmüş ekranda fare tıklaması layout koordinatına doğru çevrilmeli:
        let oturum_link = state
            .display_list()
            .iter()
            .find(|b| b.href.as_deref() == Some("http://anka.local/oturum"))
            .expect("Oturum Testi linki")
            .clone();
        let lx = oturum_link.x as i32 + 12;
        let ly = oturum_link.y as i32 + 8;
        let scaled_x = (lx * 125) / 100;
        let scaled_y = (ly * 125) / 100;
        assert!(state.is_interactive_at(scaled_x, scaled_y));
        assert_eq!(
            state.click_at(scaled_x, scaled_y),
            ClickAction::Navigate("http://anka.local/oturum".to_string())
        );
        // Sekme içi gezinmede yakınlaştırma korunmalı, Ctrl+0 ile %100'e dönmeli:
        assert_eq!(state.zoom_percent(), 125);
        assert_eq!(state.zoom_reset(), 100);

        for _ in 0..10 {
            state.zoom_out();
        }
        assert_eq!(state.zoom_percent(), 50);
    }

    #[test]
    fn test_ctrl_f_sayfa_ici_arama_ve_vurgulama() {
        let mut state = BrowserState::new("http://anka.local/hosgeldiniz", 860);
        assert!(!state.is_find_open());
        state.open_find();
        assert!(state.is_find_open());
        assert_eq!(state.find_status_text(), "Aramak icin yazin");

        state.set_find_query("anka");
        assert!(state.active().find_matches.len() >= 2);
        let first = state.active().find_matches[0];
        let second = state.active().find_matches[1];
        assert_eq!(state.find_match_kind(first), Some(true));
        assert_eq!(state.find_match_kind(second), Some(false));
        assert!(state.find_status_text().starts_with("1/"));

        assert!(state.find_next(false));
        assert_eq!(state.find_match_kind(first), Some(false));
        assert_eq!(state.find_match_kind(second), Some(true));
        assert!(state.find_status_text().starts_with("2/"));

        assert!(state.find_next(true));
        assert_eq!(state.find_match_kind(first), Some(true));

        // Sayfanın altındaki öğeye otomatik kaydırma kontrolü:
        state.set_find_query("sifirla");
        assert!(!state.active().find_matches.is_empty());
        assert!(state.scroll_y() > 0);

        // Eşleşme yoksa 0/0 raporlanmalı ve Kapat/Esc vurguları temizlemeli:
        state.set_find_query("bulunamayan_kelime_xyz");
        assert_eq!(state.find_status_text(), "0/0 eslesme");
        state.close_find();
        assert!(!state.is_find_open());
        assert_eq!(state.find_match_kind(first), None);
    }
}
