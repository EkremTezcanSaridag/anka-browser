// Anka Browser - İşletim Sisteminden Bağımsız Ortak Tarayıcı Durumu (BrowserState)
// Çoklu Sekme (Multi-Tab), Yer İmleri (Bookmarks) ve Canlı JavaScript `onclick` Etkileşimi (v0.2.7)

use anka_paint::PaintCommand;
use anka_url::Url;

use crate::{LoadedPage, html_motoru_calistir, motoru_calistir, normalize_address};

/// Kullanıcının web sayfası tuvaline tıkladığında tetiklenebilecek eylem türleri:
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickAction {
    /// Bir `<a href="...">` linkine tıklandı -> yeni adrese git!
    Navigate(String),
    /// Bir `<button onclick="...">` butonuna tıklandı -> JavaScript canlı çalıştı ve ekran yeniden çizilmeli!
    JsUpdated,
    /// Boş bir yere tıklandı
    None,
}

/// Tek bir tarayıcı sekmesinin (Tab) bağımsız canlı durumunu tutan veri yapısı:
#[derive(Clone)]
pub struct TabState {
    /// Bu sekmede açık olan sayfanın tam adresi
    pub current_url: String,
    /// Bu sekmenin canlı DOM, CSS, Görsel Önbellek ve JavaScript Runtime paketi
    pub page: LoadedPage,
    /// Bu sekmenin kendi Geri (`<-`) geçmişi yığını
    pub history: Vec<String>,
    /// Bu sekmenin kendi İleri (`->`) geçmişi yığını
    pub forward_history: Vec<String>,
    /// Bu sekmenin dikey kaydırma miktarı (piksel)
    pub scroll_y: i32,
}

/// Kullanıcının kaydettiği tek bir Yer İmi (Bookmark) kaydı:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

/// Tüm açık sekmeleri (`tabs`), aktif sekme indeksini (`active_tab`) ve
/// yer imlerini (`bookmarks`) yöneten ana tarayıcı durumu:
pub struct BrowserState {
    /// Açık olan tüm sekmelerin listesi (En az 1 sekme her zaman açıktır)
    pub tabs: Vec<TabState>,
    /// Şu an ekranda görünen aktif sekmenin `tabs` içindeki sırası (`0..tabs.len()`)
    pub active_tab: usize,
    /// Kullanıcının kaydettiği yer imleri listesi
    pub bookmarks: Vec<Bookmark>,
    /// Çizim alanının (Viewport) genişliği (piksel)
    pub viewport_width: u32,
}

impl BrowserState {
    /// Başlangıç adresi ve pencere genişliği ile ilk sekmeyi ve varsayılan yer imlerini oluşturur.
    pub fn new(initial_address: &str, viewport_width: u32) -> Self {
        let varsayilan_yer_imleri = vec![
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
        ];

        let ilk_sayfa = motoru_calistir(initial_address, viewport_width);
        let temiz_url = normalize_address(initial_address);

        Self {
            tabs: vec![TabState {
                current_url: temiz_url,
                page: ilk_sayfa,
                history: Vec::new(),
                forward_history: Vec::new(),
                scroll_y: 0,
            }],
            active_tab: 0,
            bookmarks: varsayilan_yer_imleri,
            viewport_width,
        }
    }

    /// Şu an aktif olan sekmenin (`TabState`) salt-okunur referansını döndürür.
    pub fn active(&self) -> &TabState {
        &self.tabs[self.active_tab]
    }

    /// Şu an aktif olan sekmenin (`TabState`) değiştirilebilir referansını döndürür.
    fn active_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.active_tab]
    }

    /// Aktif sekmenin URL'sini döndürür.
    pub fn current_url(&self) -> &str {
        &self.active().current_url
    }

    /// Aktif sekmenin kaydırma (`scroll_y`) konumunu döndürür.
    pub fn scroll_y(&self) -> i32 {
        self.active().scroll_y
    }

    /// Aktif sekmenin durum çubuğu mesajını (sekme ve yer imi bilgisiyle birlikte) döndürür.
    pub fn status_message(&self) -> String {
        let yildiz = if self.is_current_bookmarked() {
            "★ Yer Imi"
        } else {
            "☆"
        };
        format!(
            "[Sekme {}/{} | {}] {}",
            self.active_tab + 1,
            self.tabs.len(),
            yildiz,
            self.active().page.status_message
        )
    }

    /// Aktif sekmenin çizim listesini döndürür.
    pub fn display_list(&self) -> &[PaintCommand] {
        &self.active().page.display_list
    }

    /// Pencere başlığında gösterilecek metni üretir.
    pub fn window_title(&self) -> String {
        let tab_info = format!("[Sekme {}/{}]", self.active_tab + 1, self.tabs.len());
        match &self.active().page.page_title {
            Some(t) => format!("{t} {tab_info} — Anka Browser v0.2.7"),
            None => format!("Anka Browser v0.2.7 {tab_info} — Telek Engine"),
        }
    }

    /// Sekme çubuğundaki `index` numaralı sekme butonunun üzerinde yazacak kısa etiketi üretir.
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

    /// Yeni bir sekme (`+ Sekme`) açar, o sekmeyi aktif yapar ve verilen adresi yükler.
    pub fn new_tab(&mut self, initial_address: &str) -> usize {
        let sayfa = motoru_calistir(initial_address, self.viewport_width);
        self.tabs.push(TabState {
            current_url: normalize_address(initial_address),
            page: sayfa,
            history: Vec::new(),
            forward_history: Vec::new(),
            scroll_y: 0,
        });
        self.active_tab = self.tabs.len() - 1;
        self.active_tab
    }

    /// Verilen `index` numaralı sekmeye geçiş yapar.
    pub fn switch_tab(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab = index;
            true
        } else {
            false
        }
    }

    /// Aktif sekmeyi kapatır (`x Kapat`).
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

    /// Şu an açık olan sayfa yer imlerinde (`bookmarks`) kayıtlı mı?
    pub fn is_current_bookmarked(&self) -> bool {
        let current = self.current_url();
        self.bookmarks.iter().any(|b| b.url == current)
    }

    /// Aktif sayfayı Yer İmlerine ekler veya zaten kayıtlıysa Yer İmlerinden çıkarır.
    pub fn toggle_bookmark(&mut self) -> bool {
        let current = self.current_url().to_string();
        if current.is_empty() || current == "http://anka.local/yer-imleri" {
            return false;
        }

        if let Some(pos) = self.bookmarks.iter().position(|b| b.url == current) {
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
        }
    }

    /// `http://anka.local/yer-imleri` sayfası için kayıtlı yer imlerinden dinamik HTML üretir.
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
                "<h1>Anka Browser — Yer Imlerim ({} Kayit)</h1>",
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

    /// Aktif sekmede yeni bir adrese gider.
    pub fn navigate(&mut self, raw_address: &str, push_history: bool) {
        let clean_url = normalize_address(raw_address);
        if clean_url.is_empty() {
            return;
        }

        let current_url_before = self.active().current_url.clone();
        if push_history && !current_url_before.is_empty() && current_url_before != clean_url {
            let tab = self.active_mut();
            tab.history.push(current_url_before);
            tab.forward_history.clear();
        }

        let viewport_w = self.viewport_width;
        let loaded = if clean_url == "http://anka.local/yer-imleri" {
            let raw_http = self.build_bookmarks_http_response();
            html_motoru_calistir(&clean_url, &raw_http, viewport_w)
        } else {
            motoru_calistir(&clean_url, viewport_w)
        };

        let tab = self.active_mut();
        tab.current_url = clean_url;
        tab.page = loaded;
        tab.scroll_y = 0;
    }

    /// Aktif sekmede Geri (`<-`) butonuna basıldığında bir önceki sayfaya döner.
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

    /// Aktif sekmede İleri (`->`) butonuna basıldığında bir sonraki sayfaya gider.
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

    /// Aktif sekmeyi yeniden yükler (`Yenile`).
    pub fn reload(&mut self) {
        let current = self.current_url().to_string();
        self.navigate(&current, false);
    }

    /// Aktif sekmeyi fare tekerleği ile dikey kaydırır (`delta_y` piksel kadar).
    pub fn scroll_by(&mut self, delta_y: i32) {
        let tab = self.active_mut();
        tab.scroll_y = (tab.scroll_y + delta_y).max(0);
    }

    /// Tuval üzerindeki `(canvas_x, canvas_y)` koordinatında tıklanabilir bir link (`href`)
    /// veya JavaScript butonu (`onclick`) olup olmadığını kontrol eder (İmleç değiştirme için).
    #[allow(dead_code)]
    pub fn is_interactive_at(&self, canvas_x: i32, canvas_y: i32) -> bool {
        let tab = self.active();
        tab.page.display_list.iter().any(|komut| {
            if komut.href.is_some() || komut.onclick.is_some() {
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

    /// Tuval (Canvas) üzerindeki `(canvas_x, canvas_y)` noktasına tıklandığında:
    /// 1. Eğer bir `<button onclick="...">` (veya `onclick` taşıyan öğe) varsa JavaScript kodunu
    ///    canlı olarak `anka-js` üzerinden çalıştırır, DOM'u günceller ve `ClickAction::JsUpdated` döner!
    /// 2. Eğer bir `<a href="...">` linki varsa tam adresi çözüp `ClickAction::Navigate(url)` döner!
    pub fn click_at(&mut self, canvas_x: i32, canvas_y: i32) -> ClickAction {
        let tab = self.active();
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
                if let Some(js_code) = &komut.onclick {
                    found_onclick = Some(js_code.clone());
                    break;
                }
                if let Some(href) = &komut.href {
                    found_href = Some(href.clone());
                    break;
                }
            }
        }

        if let Some(js_code) = found_onclick {
            let vw = self.viewport_width;
            self.active_mut().page.run_inline_js(&js_code, vw);
            return ClickAction::JsUpdated;
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

        // 1) `<script>` sayfa açılır açılmaz `js_sayac` yazısını JS ile değiştirmiş olmalı!
        assert!(
            state
                .display_list()
                .iter()
                .any(|k| k.text.contains("Anka JS Hazir! Tiklama Sayisi: 0"))
        );

        // 2) `+1 Artir (JS)` butonunun koordinatını bulup tıkla!
        let js_btn = state
            .display_list()
            .iter()
            .find(|k| k.onclick.as_deref() == Some("artir()"))
            .expect("Ana sayfada artir() JS butonu olmali")
            .clone();

        let action = state.click_at(js_btn.x as i32 + 10, js_btn.y as i32 + 5);
        assert_eq!(action, ClickAction::JsUpdated);

        // Tıklama sonrası DOM yeniden çizilmiş ve sayaç "1" olmuş olmalı!
        assert!(
            state
                .display_list()
                .iter()
                .any(|k| k.text.contains("Tiklama Sayisi: 1"))
        );

        // 3) Bir linke tıklama testini doğrula:
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
    fn test_yer_imleri_ekleme_cikarma_ve_sayfasi() {
        let mut state = BrowserState::new("http://anka.local/ozel-makale", 860);
        assert!(!state.is_current_bookmarked());

        assert!(state.toggle_bookmark());
        assert!(state.is_current_bookmarked());

        state.navigate("http://anka.local/yer-imleri", true);
        assert!(
            state
                .display_list()
                .iter()
                .any(|k| k.href.as_deref() == Some("http://anka.local/ozel-makale"))
        );

        state.go_back();
        assert!(!state.toggle_bookmark());
        assert!(!state.is_current_bookmarked());
    }
}
