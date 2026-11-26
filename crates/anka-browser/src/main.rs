// Anka Browser - Ana Tarayıcı Uygulaması (7 İstasyonlu Telek Engine + Linux/Windows GUI)

use std::collections::HashMap;

use anka_css::{CssRule, parse_css};
use anka_html::{
    DomNode, NodeType, extract_scripts, extract_styles, extract_stylesheet_links, extract_title,
    parse_html,
};
use anka_js::JsRuntime;
use anka_layout::{compute_layout, compute_layout_with_images, resolve_body_background};
use anka_net::{
    NetError, fetch_with_shield, parse_http_response,
    shield::{self, ShieldStats},
};
use anka_paint::{DecodedImage, PaintCommand, RgbColor, build_display_list};
use anka_url::Url;

#[cfg(any(windows, target_os = "linux", test))]
mod browser_state;

#[cfg(windows)]
mod win32_gui;

#[cfg(target_os = "linux")]
mod linux_gui;

/// Bir sayfanın 7 istasyondan geçtikten sonraki canlı belgesi (`LoadedPage`):
/// - DOM ağacını, CSS kurallarını, görsel önbelleğini ve canlı JavaScript ortamını saklar!
/// - Böylece kullanıcı bir `<button onclick="...">` butonuna bastığında internetten tekrar
///   indirme yapmadan doğrudan bellekteki DOM'u değiştirip sayfayı anında yeniden çizer!
#[derive(Clone)]
pub struct LoadedPage {
    pub parsed_url: Option<Url>,
    pub dom: Option<DomNode>,
    pub css_rules: Vec<CssRule>,
    pub js_runtime: JsRuntime,
    pub image_cache: HashMap<String, DecodedImage>,
    pub page_title: Option<String>,
    pub page_bg: Option<RgbColor>,
    pub status_message: String,
    pub display_list: Vec<PaintCommand>,
    pub status_code: u16,
    pub body_len: usize,
    pub shield_stats: ShieldStats,
}

impl LoadedPage {
    /// Hata durumunda boş bir sayfa nesnesi oluşturur.
    fn error(message: String) -> Self {
        Self {
            parsed_url: None,
            dom: None,
            css_rules: Vec::new(),
            js_runtime: JsRuntime::new(),
            image_cache: HashMap::new(),
            page_title: None,
            page_bg: None,
            status_message: message,
            display_list: Vec::new(),
            status_code: 0,
            body_len: 0,
            shield_stats: ShieldStats::default(),
        }
    }

    /// Mevcut canlı DOM ağacı (`self.dom`) değiştiğinde (örneğin bir `onclick` JS kodu çalıştığında)
    /// ağa tekrar gitmeden yalnızca `anka-layout -> anka-paint` aşamasını ışık hızında yeniden çalıştırır!
    pub fn re_render(&mut self, viewport_width: u32) {
        let Some(dom) = &self.dom else {
            return;
        };

        // `document.title` JS tarafından değiştirildiyse onu öncelikli kullan:
        self.page_title = self
            .js_runtime
            .document_title
            .clone()
            .or_else(|| extract_title(dom));

        let image_sizes: HashMap<String, (u32, u32)> = self
            .image_cache
            .iter()
            .map(|(k, img)| (k.clone(), (img.width, img.height)))
            .collect();
        let yerlesim_kutulari =
            compute_layout_with_images(dom, &self.css_rules, viewport_width, &image_sizes);
        self.page_bg = resolve_body_background(dom, &self.css_rules)
            .and_then(|bg| RgbColor::try_from_css(&bg));
        let mut cizim_listesi = build_display_list(&yerlesim_kutulari);

        // Görselleri önbellekten bağla (yeniden ağdan indirmeye gerek yok!)
        for komut in &mut cizim_listesi {
            if let Some(src) = &komut.image_src {
                if let Some(cached) = self.image_cache.get(src) {
                    komut.image = Some(cached.clone());
                } else if src.contains("anka-logo") {
                    let banner = DecodedImage::generate_anka_banner(komut.width, komut.height);
                    self.image_cache.insert(src.clone(), banner.clone());
                    komut.image = Some(banner);
                }
            }
        }

        let protokol_etiketi = match self.parsed_url.as_ref().map(|u| u.scheme.as_str()) {
            Some("https") => "HTTPS/TLS",
            Some("file") => "FILE",
            _ => "HTTP",
        };

        let js_notu = if let Some(son_log) = self.js_runtime.logs.last() {
            format!(" | JS: {son_log}")
        } else {
            String::new()
        };

        let kalkan_notu = if self.shield_stats.total_blocked() > 0 {
            format!(" | Kalkan: {} engellendi", self.shield_stats.total_blocked())
        } else {
            String::new()
        };

        self.status_message = format!(
            "{} {} | {} bayt | {} kutu | Telek Engine v0.2.11{}{}",
            protokol_etiketi,
            self.status_code,
            self.body_len,
            cizim_listesi.len(),
            kalkan_notu,
            js_notu
        );
        self.display_list = cizim_listesi;
    }

    /// Kullanıcının tıkladığı bir öğedeki `onclick="..."` JavaScript kodunu canlı çalıştırır
    /// ve değişen DOM'u yeniden çizer!
    pub fn run_inline_js(&mut self, js_code: &str, viewport_width: u32) {
        if let Some(dom) = self.dom.as_mut() {
            self.js_runtime.eval(js_code, dom);
        }
        self.re_render(viewport_width);
    }
}

/// Akıllı adres çubuğu:
/// - `/home/.../dosya.html` veya `C:\...\dosya.html` gibi yerel dosya yolu girildiyse otomatik `file://` ekler.
/// - Protokol yazılmayan web adreslerinin başına otomatik `https://` ekler.
pub fn normalize_address(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains("://") {
        return trimmed.to_string();
    }
    if trimmed.starts_with('/') {
        return format!("file://{trimmed}");
    }
    let b = trimmed.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
    {
        let normalized_slashes = trimmed.replace('\\', "/");
        return format!("file:///{normalized_slashes}");
    }
    format!("https://{trimmed}")
}

/// Verilen ham HTML/HTTP yanıtını 7 istasyondan (HTML -> JS -> CSS -> Layout -> Paint) geçirip canlı `LoadedPage` üretir.
pub fn html_motoru_calistir(
    sayfa_url_str: &str,
    ham_http_yaniti: &str,
    viewport_width: u32,
) -> LoadedPage {
    html_motoru_calistir_kalkanli(sayfa_url_str, ham_http_yaniti, viewport_width, true)
}

pub fn html_motoru_calistir_kalkanli(
    sayfa_url_str: &str,
    ham_http_yaniti: &str,
    viewport_width: u32,
    shield_enabled: bool,
) -> LoadedPage {
    let Ok(url) = Url::parse(sayfa_url_str) else {
        return LoadedPage::error("Gecersiz dahili URL".to_string());
    };
    let Ok(yanit) = parse_http_response(ham_http_yaniti) else {
        return LoadedPage::error("Dahili HTTP yanit hatasi".to_string());
    };
    yaniti_canli_sayfaya_donustur(&url, &yanit, viewport_width, shield_enabled, ShieldStats::default())
}

/// DOM ağacı içindeki reklam kutularını, 1x1 gizli izleme piksellerini ve izleyici URL'li
/// `<script>` / `<iframe>` / `<img>` öğelerini Anka Kalkan kurallarına göre temizler.
fn dom_kalkan_temizle(node: &mut DomNode, base_url: &Url, stats: &mut ShieldStats) {
    let mut kept = Vec::with_capacity(node.children.len());
    for mut child in std::mem::take(&mut node.children) {
        if let NodeType::Element(tag_name) = &child.node_type {
            if tag_name == "img" {
                if shield::is_tracking_pixel(
                    child.get_attribute("width"),
                    child.get_attribute("height"),
                ) {
                    stats.blocked_dom_nodes += 1;
                    continue;
                }
                if let Some(src) = child.get_attribute("src")
                    && let Ok(resolved) = base_url.resolve(src)
                    && shield::is_blocked_url(&resolved)
                {
                    stats.record_request(&resolved.host);
                    continue;
                }
            }

            if matches!(tag_name.as_str(), "script" | "iframe")
                && let Some(src) = child.get_attribute("src")
                && let Ok(resolved) = base_url.resolve(src)
                && shield::is_blocked_url(&resolved)
            {
                stats.record_request(&resolved.host);
                continue;
            }

            if shield::is_cosmetic_ad_element(
                child.get_attribute("class"),
                child.get_attribute("id"),
            ) {
                stats.blocked_dom_nodes += 1;
                continue;
            }

            if tag_name == "a"
                && let Some(href) = child.get_attribute("href")
            {
                let (clean_href, stripped) = shield::strip_tracking_params(href);
                if stripped > 0 {
                    stats.stripped_params += stripped;
                    child.set_attribute("href", &clean_href);
                }
            }
        }

        dom_kalkan_temizle(&mut child, base_url, stats);
        kept.push(child);
    }
    node.children = kept;
}

fn yaniti_canli_sayfaya_donustur(
    url: &Url,
    yanit: &anka_net::HttpResponse,
    viewport_width: u32,
    shield_enabled: bool,
    mut shield_stats: ShieldStats,
) -> LoadedPage {
    // İSTASYON 3: anka-html (HTML Metnini DOM Ağacına Çevirme)
    let mut dom_agaci = parse_html(&yanit.body);

    if shield_enabled {
        dom_kalkan_temizle(&mut dom_agaci, url, &mut shield_stats);
    }

    // İSTASYON 7: anka-js (Sayfadaki tüm `<script>...</script>` bloklarını canlı DOM üzerinde çalıştır!)
    let mut js_runtime = JsRuntime::new();
    js_runtime.document_title = extract_title(&dom_agaci);
    let scripts = extract_scripts(&dom_agaci);
    for script_code in scripts {
        js_runtime.eval(&script_code, &mut dom_agaci);
    }

    // İSTASYON 4: anka-css (Varsayılan Stiller + Harici CSS + Sayfa İçi `<style>`)
    let varsayilan_css = "
        h1 { color: red; font-size: 30px; }
        h2 { color: blue; font-size: 22px; }
        p  { color: black; font-size: 16px; }
        li { color: black; font-size: 16px; }
        a  { color: blue; font-size: 16px; }
        button { color: #0f172a; background-color: #e2e8f0; border: 1px solid #94a3b8; font-size: 15px; }
    ";
    let mut css_kurallari = parse_css(varsayilan_css);

    if url.host != "anka.local" {
        for css_href in extract_stylesheet_links(&dom_agaci).into_iter().take(3) {
            if let Ok(css_url) = url.resolve(&css_href) {
                match fetch_with_shield(&css_url, shield_enabled) {
                    Ok(css_resp) => css_kurallari.extend(parse_css(&css_resp.body)),
                    Err(NetError::BlockedByShield(host)) => shield_stats.record_request(&host),
                    Err(_) => {}
                }
            }
        }
    }

    let sayfa_css = extract_styles(&dom_agaci);
    if !sayfa_css.trim().is_empty() {
        css_kurallari.extend(parse_css(&sayfa_css));
    }

    // İSTASYON 5 & 6: İlk Layout + Görsel İndirme Önbelleği
    let yerlesim_kutulari = compute_layout(&dom_agaci, &css_kurallari, viewport_width);
    let mut image_cache = HashMap::new();
    let mut indirilen_gorsel = 0usize;

    for kutu in &yerlesim_kutulari {
        if let Some(src) = &kutu.image_src {
            if image_cache.contains_key(src) {
                continue;
            }
            if src.contains("anka-logo") {
                image_cache.insert(
                    src.clone(),
                    DecodedImage::generate_anka_banner(kutu.width, kutu.height),
                );
                continue;
            }
            if indirilen_gorsel < 8
                && url.host != "anka.local"
                && let Ok(img_url) = url.resolve(src)
            {
                match fetch_with_shield(&img_url, shield_enabled) {
                    Ok(img_resp) => {
                        if let Some(decoded) = DecodedImage::decode_bytes(&img_resp.raw_body) {
                            image_cache.insert(src.clone(), decoded);
                            indirilen_gorsel += 1;
                        }
                    }
                    Err(NetError::BlockedByShield(host)) => shield_stats.record_request(&host),
                    Err(_) => {}
                }
            }
        }
    }

    let mut page = LoadedPage {
        parsed_url: Some(url.clone()),
        dom: Some(dom_agaci),
        css_rules: css_kurallari,
        js_runtime,
        image_cache,
        page_title: None,
        page_bg: None,
        status_message: String::new(),
        display_list: Vec::new(),
        status_code: yanit.status_code,
        body_len: yanit.body.len(),
        shield_stats,
    };
    page.re_render(viewport_width);
    page
}

/// 7 İSTASYONLUK MOTORU ATEŞLEYEN ANA FONKSİYON (`Anka Kalkan` varsayılan olarak açık):
/// `anka-url -> anka-net (Shield) -> anka-html -> anka-js -> anka-css -> anka-layout -> anka-paint`
pub fn motoru_calistir(hedef_adres: &str, viewport_width: u32) -> LoadedPage {
    motoru_calistir_kalkanli(hedef_adres, viewport_width, true)
}

pub fn motoru_calistir_kalkanli(
    hedef_adres: &str,
    viewport_width: u32,
    shield_enabled: bool,
) -> LoadedPage {
    let mut hedef = normalize_address(hedef_adres);
    let mut initial_stats = ShieldStats::default();
    if shield_enabled {
        let (clean_url, removed) = shield::strip_tracking_params(&hedef);
        hedef = clean_url;
        initial_stats.stripped_params += removed;
    }

    // İSTASYON 1: anka-url (Adresi Doğrama)
    let url = match Url::parse(&hedef) {
        Ok(u) => u,
        Err(hata) => {
            return LoadedPage::error(format!("Hata [anka-url] Gecersiz adres ({hedef}): {hata:?}"));
        }
    };

    // İSTASYON 2: anka-net (İnternetten, Yerel Diskten veya Dahili Sayfadan HTML İndirme)
    let yanit = if url.host == "anka.local" {
        let dahili_http = if url.path == "/kalkan" {
            concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: text/html\r\n\r\n",
                "<html><head><title>Anka Kalkan Testi</title><style>",
                "h1 { color: #0f766e; background-color: #f0fdfa; border: 1px solid #99f6e4; padding: 8px; text-align: center; }",
                ".advertisement { color: #991b1b; background-color: #fee2e2; border: 1px solid #ef4444; padding: 10px; }",
                ".bilgi { color: #1e293b; background-color: #f8fafc; border: 1px solid #cbd5e1; padding: 10px; }",
                "</style></head><body>",
                "<h1>Anka Kalkan (Reklam &amp; Izleyici Engelleyici) Test Sayfasi</h1>",
                "<div class=\"bilgi\"><p>Asagida gizli 1x1 izleme pikseli, izleyici script ve bir reklam kutusu vardir. Ust cubuktaki Kalkan dugmesine basarak farki aninda gorebilirsiniz!</p>",
                "<p><a href=\"https://example.com/?utm_source=anka&amp;fbclid=izleyici123\">Izleyicisiz Temiz Link Ornegi</a></p></div>",
                "<div class=\"advertisement\"><p>REKLAM ALANI: Kalkan Acikken Bu Kutu Gizlenir!</p></div>",
                "<img src=\"https://google-analytics.com/collect.gif\" alt=\"Gizli Izleyici\" width=\"1\" height=\"1\">",
                "<script src=\"https://doubleclick.net/ads/tracker.js\"></script>",
                "<p><a href=\"http://anka.local/hosgeldiniz\">Ana Sayfaya Don</a></p>",
                "</body></html>"
            )
        } else {
            concat!(
                "HTTP/1.1 200 OK\r\n",
                "Content-Type: text/html\r\n\r\n",
                "<html><head><title>Ana Sayfa</title><style>",
                "h1 { color: #dc2626; background-color: #fef2f2; border: 1px solid #fecaca; font-size: 30px; padding: 6px; text-align: center; }",
                "h2 { color: #1d4ed8; font-size: 20px; }",
                "p  { color: #334155; font-size: 16px; }",
                "li { color: #0f766e; font-size: 16px; }",
                ".pill { color: #1e40af; background-color: #dbeafe; border: 1px solid #93c5fd; font-size: 15px; }",
                ".jsbtn { color: #ffffff; background-color: #16a34a; border: 1px solid #15803d; font-size: 15px; }",
                ".jsreset { color: #ffffff; background-color: #dc2626; border: 1px solid #b91c1c; font-size: 15px; }",
                ".satir { display: flex; gap: 14px; }",
                ".kart { color: #065f46; background-color: #ecfdf5; border: 1px solid #6ee7b7; padding: 10px; font-size: 15px; }",
                "#duyuru { color: #9a3412; background-color: #ffedd5; border: 1px solid #fdba74; padding: 10px; font-size: 15px; }",
                "</style></head><body>",
                "<h1>Anka Browser v0.2.11 — Anka Kalkan (Reklam &amp; Izleyici Engelleyici)!</h1>",
                "<nav>",
                "<a class=\"pill\" href=\"https://example.com\">HTTPS Example.com</a>",
                "<a class=\"pill\" href=\"http://info.cern.ch\">CERN (1991)</a>",
                "<a class=\"pill\" href=\"http://anka.local/yer-imleri\">Kalici Yer Imlerim</a>",
                "<a class=\"pill\" href=\"http://anka.local/kalkan\">Kalkan Testi</a>",
                "<a class=\"pill\" href=\"https://tr.wikipedia.org\">Vikipedi TR</a>",
                "</nav>",
                "<hr>",
                "<form action=\"https://tr.wikipedia.org/w/index.php\">",
                "<input type=\"text\" name=\"search\" placeholder=\"Vikipedi TR'de aramak icin tiklayip yazin...\">",
                "<input type=\"submit\" value=\"Vikipedi Ara\">",
                "</form>",
                "<img src=\"http://anka.local/anka-logo.bmp\" alt=\"Anka Telek Engine Piksel Amblemi\" width=\"380\" height=\"110\">",
                "<h2>7. Istasyon (Anka JS), Anka Kalkan, Kalici Yer Imleri ve Canli Formlar!</h2>",
                "<div class=\"satir\">",
                "<div class=\"kart\">",
                "<p id=\"js_sayac\">Yukleniyor...</p>",
                "<button class=\"jsbtn\" onclick=\"artir()\">+1 Artir (JS)</button>",
                "<button class=\"jsreset\" onclick=\"sifirla()\">Sifirla</button>",
                "</div>",
                "<div id=\"duyuru\"><p>Sol kutudaki yesil butona tikladiginizda sifirdan yazdigimiz anka-js motoru calisip DOM agacini aninda gunceller!</p></div>",
                "</div>",
                "<script>",
                "let tiklama = 0;",
                "document.getElementById(\"js_sayac\").innerText = \"Anka JS Hazir! Tiklama Sayisi: 0\";",
                "console.log(\"Anka JS v0.2.7 baslatildi!\");",
                "function artir() {",
                "  tiklama++;",
                "  document.getElementById(\"js_sayac\").innerText = \"Tiklama Sayisi: \" + tiklama;",
                "  console.log(\"Butona tiklandi -> \" + tiklama);",
                "  if (tiklama >= 5) {",
                "    document.getElementById(\"js_sayac\").style.color = \"#dc2626\";",
                "  }",
                "}",
                "function sifirla() {",
                "  tiklama = 0;",
                "  document.getElementById(\"js_sayac\").innerText = \"Sayac Sifirlandi: 0\";",
                "  document.getElementById(\"js_sayac\").style.color = \"#065f46\";",
                "  console.log(\"Sayac sifirlandi\");",
                "}",
                "</script>",
                "</body></html>"
            )
        };
        match parse_http_response(dahili_http) {
            Ok(r) => r,
            Err(e) => {
                return LoadedPage::error(format!("Hata [anka-net] Dahili yanit hatasi: {e:?}"));
            }
        }
    } else {
        match fetch_with_shield(&url, shield_enabled) {
            Ok(resp) => resp,
            Err(NetError::BlockedByShield(host)) => {
                initial_stats.record_request(&host);
                let blok_html = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                     <html><head><title>Anka Kalkan Engelledi</title>\
                     <style>.uyari {{ color: #991b1b; background-color: #fef2f2; border: 1px solid #ef4444; padding: 14px; }}</style>\
                     </head><body><div class=\"uyari\"><h1>Anka Kalkan Bu Siteyi Engelledi</h1>\
                     <p>{host} alan adi bilinen bir reklam veya izleyici agi oldugu icin baglanti acilmadi.</p>\
                     <p><a href=\"http://anka.local/hosgeldiniz\">Ana Sayfaya Don</a></p></div></body></html>"
                );
                return yaniti_canli_sayfaya_donustur(
                    &url,
                    &parse_http_response(&blok_html).unwrap(),
                    viewport_width,
                    shield_enabled,
                    initial_stats,
                );
            }
            Err(hata) => {
                return LoadedPage::error(format!(
                    "Hata [anka-net] Baglanti kurulamadi ({hedef}): {hata:?}"
                ));
            }
        }
    };

    yaniti_canli_sayfaya_donustur(&url, &yanit, viewport_width, shield_enabled, initial_stats)
}

fn main() {
    let mut headless = false;
    let mut baslangic_adresi: Option<String> = None;
    for arg in std::env::args().skip(1) {
        if arg == "--headless" {
            headless = true;
        } else if baslangic_adresi.is_none() {
            baslangic_adresi = Some(arg);
        }
    }
    let baslangic_adresi =
        baslangic_adresi.unwrap_or_else(|| "http://anka.local/hosgeldiniz".to_string());

    if !headless {
        // Windows'ta doğrudan gerçek yerel masaüstü penceresini (Win32 GDI) aç!
        #[cfg(windows)]
        {
            win32_gui::pencereyi_baslat(&baslangic_adresi);
            return;
        }

        // Linux'ta (Milis, Ubuntu, Fedora, Arch vb.) masaüstü ekranı varsa yerel GTK3 + Cairo penceresini aç!
        #[cfg(target_os = "linux")]
        {
            if linux_gui::pencereyi_baslat(&baslangic_adresi) {
                return;
            }
        }
    }

    // Ekransız (Headless / `--headless` / CI) ortamda terminal önizlemesi çalışır
    let sayfa = motoru_calistir(&baslangic_adresi, 860);
    println!(
        "Anka Browser v0.2.11 ({}) [{}] | page_bg={:?} -> {}",
        baslangic_adresi,
        sayfa.page_title.as_deref().unwrap_or("Adsız"),
        sayfa.page_bg,
        sayfa.status_message
    );
    for komut in &sayfa.display_list {
        println!(
            "[x:{:>2}, y:{:>3}, w:{:>3}, h:{:>3} | {:>2}px | fg({:>3},{:>3},{:>3}) bg({:?})] <{}> {}",
            komut.x,
            komut.y,
            komut.width,
            komut.height,
            komut.font_size,
            komut.color.r,
            komut.color.g,
            komut.color.b,
            komut.bg_color,
            komut.tag,
            komut.text
        );
    }
}
