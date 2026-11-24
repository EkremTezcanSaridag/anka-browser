// Anka Browser - İşletim Sisteminden Bağımsız Ortak Tarayıcı Durumu (BrowserState)
// Hem Windows (Win32 GDI) hem de tüm Linux dağıtımları (GTK3 + Cairo)
// gezinme geçmişini (Geri / İleri), kaydırma konumunu (scroll_y) ve link tıklama (hit-test)
// hesaplamalarını bu ortak modül üzerinden yürütür.

use anka_paint::PaintCommand;
use anka_url::Url;

use crate::{motoru_calistir, normalize_address};

/// Tarayıcı penceresinin o anki durumunu tutan ortak veri yapısı:
pub struct BrowserState {
    /// Şu an açık olan sayfanın tam adresi (örn: "https://example.com")
    pub current_url: String,
    /// Sayfanın `<title>` etiketinden okunan başlığı
    pub page_title: Option<String>,
    /// Geri (`<-`) butonu için ziyaret edilen önceki sayfaların yığını (Stack)
    pub history: Vec<String>,
    /// İleri (`->`) butonu için geri gelinen sayfaların yığını (Stack)
    pub forward_history: Vec<String>,
    /// Sayfanın dikey kaydırma miktarı (piksel)
    pub scroll_y: i32,
    /// Alt durum çubuğunda gösterilen özet bilgi
    pub status_message: String,
    /// `anka-paint` tarafından üretilen ve ekrana çizilecek olan komut listesi
    pub display_list: Vec<PaintCommand>,
    /// Çizim alanının (Viewport) genişliği (piksel)
    pub viewport_width: u32,
}

impl BrowserState {
    /// Başlangıç adresi ve pencere genişliği ile yeni bir tarayıcı durumu oluşturur
    /// ve ilk sayfayı 6 istasyonlu Telek Engine üzerinden geçirerek yükler.
    pub fn new(initial_address: &str, viewport_width: u32) -> Self {
        let clean_url = normalize_address(initial_address);
        let (page_title, status_message, display_list) =
            motoru_calistir(&clean_url, viewport_width);

        Self {
            current_url: clean_url,
            page_title,
            history: Vec::new(),
            forward_history: Vec::new(),
            scroll_y: 0,
            status_message,
            display_list,
            viewport_width,
        }
    }

    /// Pencere başlığında gösterilecek metni üretir.
    pub fn window_title(&self) -> String {
        match &self.page_title {
            Some(t) => format!("{t} — Anka Browser v0.2.3"),
            None => "Anka Browser v0.2.3 — Telek Engine".to_string(),
        }
    }

    /// Yeni bir adrese gider. `push_history = true` ise mevcut adresi Geri (`history`)
    /// yığınına ekler ve İleri (`forward_history`) yığınını temizler.
    pub fn navigate(&mut self, raw_address: &str, push_history: bool) {
        let clean_url = normalize_address(raw_address);
        if clean_url.is_empty() {
            return;
        }

        if push_history && !self.current_url.is_empty() && self.current_url != clean_url {
            self.history.push(self.current_url.clone());
            self.forward_history.clear();
        }

        let (page_title, status_message, display_list) =
            motoru_calistir(&clean_url, self.viewport_width);

        self.current_url = clean_url;
        self.page_title = page_title;
        self.scroll_y = 0;
        self.status_message = status_message;
        self.display_list = display_list;
    }

    /// Geri (`<-`) butonuna basıldığında bir önceki sayfaya döner.
    /// Geri gidilecek sayfa varsa `true`, yoksa `false` döner.
    pub fn go_back(&mut self) -> bool {
        if let Some(prev_url) = self.history.pop() {
            self.forward_history.push(self.current_url.clone());
            let prev_clone = prev_url.clone();
            self.navigate(&prev_clone, false);
            true
        } else {
            false
        }
    }

    /// İleri (`->`) butonuna basıldığında bir sonraki sayfaya gider.
    /// İleri gidilecek sayfa varsa `true`, yoksa `false` döner.
    pub fn go_forward(&mut self) -> bool {
        if let Some(next_url) = self.forward_history.pop() {
            self.history.push(self.current_url.clone());
            let next_clone = next_url.clone();
            self.navigate(&next_clone, false);
            true
        } else {
            false
        }
    }

    /// Mevcut sayfayı yeniden yükler (`Yenile`).
    pub fn reload(&mut self) {
        let current = self.current_url.clone();
        self.navigate(&current, false);
    }

    /// Fare tekerleği ile sayfayı dikey kaydırır (`delta_y` piksel kadar).
    pub fn scroll_by(&mut self, delta_y: i32) {
        self.scroll_y = (self.scroll_y + delta_y).max(0);
    }

    /// Tuval (Canvas) üzerindeki `(canvas_x, canvas_y)` noktasında tıklanabilir bir `<a href="...">`
    /// link kutusu olup olmadığını kontrol eder (Hit-Testing).
    /// Link varsa bağıl adresi (`href`) mevcut sayfa adresine göre tam adrese (`https://...`) çevirip döndürür.
    pub fn link_at(&self, canvas_x: i32, canvas_y: i32) -> Option<String> {
        let base_url = Url::parse(&self.current_url).ok()?;

        for komut in &self.display_list {
            if let Some(href) = &komut.href {
                let draw_x = komut.x as i32 + 4;
                let draw_right = draw_x + komut.width as i32;
                let draw_y = komut.y as i32 - self.scroll_y;
                let draw_bottom = draw_y + komut.height as i32;

                if canvas_x >= draw_x
                    && canvas_x <= draw_right
                    && canvas_y >= draw_y
                    && canvas_y <= draw_bottom
                    && let Ok(resolved) = base_url.resolve(href)
                {
                    let port_part = resolved
                        .port
                        .map(|p| format!(":{p}"))
                        .unwrap_or_default();
                    let query_part = resolved
                        .query
                        .map(|q| format!("?{q}"))
                        .unwrap_or_default();
                    return Some(format!(
                        "{}://{}{}{}{}",
                        resolved.scheme, resolved.host, port_part, resolved.path, query_part
                    ));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gezinme_gecmisi_geri_ve_ileri() {
        let mut state = BrowserState::new("http://anka.local/sayfa1", 860);
        assert_eq!(state.current_url, "http://anka.local/sayfa1");
        assert!(state.history.is_empty());

        // 2. sayfaya git
        state.navigate("http://anka.local/sayfa2", true);
        assert_eq!(state.current_url, "http://anka.local/sayfa2");
        assert_eq!(state.history, vec!["http://anka.local/sayfa1".to_string()]);

        // Geri (`<-`) gel
        assert!(state.go_back());
        assert_eq!(state.current_url, "http://anka.local/sayfa1");
        assert_eq!(
            state.forward_history,
            vec!["http://anka.local/sayfa2".to_string()]
        );

        // İleri (`->`) git
        assert!(state.go_forward());
        assert_eq!(state.current_url, "http://anka.local/sayfa2");
    }

    #[test]
    fn test_link_tiklama_koordinat_yakalama() {
        let state = BrowserState::new("http://anka.local/hosgeldiniz", 860);
        // Dahili ana sayfadaki ilk `.pill` linkinin (`https://example.com`) koordinatını bul:
        let ilk_link = state
            .display_list
            .iter()
            .find(|k| k.href.as_deref() == Some("https://example.com"))
            .expect("Ana sayfada example.com linki olmali");

        let tiklama_x = ilk_link.x as i32 + 10;
        let tiklama_y = ilk_link.y as i32 + 5;

        assert_eq!(
            state.link_at(tiklama_x, tiklama_y),
            Some("https://example.com/".to_string())
        );
        // Boş bir yere tıklandığında None dönmeli
        assert_eq!(state.link_at(5, 5), None);
    }
}
