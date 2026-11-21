// Anka Browser - Ana Tarayıcı Uygulaması (6 İstasyonlu Motor + Yerel Masaüstü Penceresi)

use anka_css::parse_css;
use anka_html::{extract_styles, extract_stylesheet_links, extract_title, parse_html};
use anka_layout::compute_layout;
use anka_net::{fetch, parse_http_response};
use anka_paint::{PaintCommand, build_display_list};
use anka_url::Url;

#[cfg(windows)]
mod win32_gui;

/// Akıllı adres çubuğu: Kullanıcı protokol (`http://` veya `https://`) yazmayı unuttuysa
/// otomatik olarak başına `https://` ekler.
pub fn normalize_address(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    }
}

/// 6 İSTASYONLUK MOTORU ATEŞLEYEN ORTAK FONKSİYON:
/// Verilen adresi alır: anka-url -> anka-net -> anka-html -> anka-css -> anka-layout -> anka-paint
/// sırasıyla işleyip (Sayfa Başlığı, Durum Mesajı, Çizim Listesi) döndürür.
pub fn motoru_calistir(
    hedef_adres: &str,
    viewport_width: u32,
) -> (Option<String>, String, Vec<PaintCommand>) {
    let hedef = normalize_address(hedef_adres);

    // İSTASYON 1: anka-url (Adresi Doğrama)
    let url = match Url::parse(&hedef) {
        Ok(u) => u,
        Err(hata) => {
            return (
                None,
                format!("Hata [anka-url] Gecersiz adres ({hedef}): {hata:?}"),
                Vec::new(),
            );
        }
    };

    // İSTASYON 2: anka-net (İnternetten veya Dahili Sayfadan HTML İndirme)
    let yanit = if url.host == "anka.local" {
        let dahili_http = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: text/html\r\n\r\n",
            "<html><head><title>Ana Sayfa</title><style>",
            "h1 { color: #dc2626; background-color: #fef2f2; font-size: 30px; }",
            "h2 { color: #1d4ed8; font-size: 20px; }",
            "p  { color: #334155; font-size: 16px; }",
            "li { color: #0f766e; font-size: 16px; }",
            "</style></head><body>",
            "<h1>Anka Browser v0.2.1 — Telek Engine</h1>",
            "<h2>Okunabilir Gercek Web Surumu Hazir!</h2>",
            "<p>Bu surumde uzun paragraflar pencere genisligine gore otomatik olarak alt satira kaydirilir (Word Wrap), boylece hicbir yazi ekranin sagindan disari tasmaz.</p>",
            "<ul>",
            "<li>Otomatik Satir Kaydirma (Word Wrap) ve Liste Maddeleri</li>",
            "<li>Kutu Arka Plan Renkleri (background-color) ve Harici CSS Indirme</li>",
            "<li>Adres Cubugunda Enter Tusu ve Link Uzerinde El Imleci</li>",
            "</ul>",
            "<a href=\"https://example.com\">Ornek 1: Sifreli HTTPS Example.com sayfasina gitmek icin bu linke tiklayin!</a>",
            "<a href=\"http://info.cern.ch\">Ornek 2: CERN (Dunyanin Ilk Web Sitesi) sayfasina gitmek icin tiklayin!</a>",
            "</body></html>"
        );
        match parse_http_response(dahili_http) {
            Ok(r) => r,
            Err(e) => {
                return (
                    None,
                    format!("Hata [anka-net] Dahili yanit hatasi: {e:?}"),
                    Vec::new(),
                );
            }
        }
    } else {
        match fetch(&url) {
            Ok(resp) => resp,
            Err(hata) => {
                return (
                    None,
                    format!("Hata [anka-net] Baglanti kurulamadi ({hedef}): {hata:?}"),
                    Vec::new(),
                );
            }
        }
    };

    // İSTASYON 3: anka-html (HTML Metnini DOM Ağacına Çevirme ve <title> Çıkarma)
    let dom_agaci = parse_html(&yanit.body);
    let sayfa_basligi = extract_title(&dom_agaci);

    // İSTASYON 4: anka-css (Varsayılan Stiller + Sayfa İçi <style> + Harici <link rel="stylesheet">)
    let varsayilan_css = "
        h1 { color: red; font-size: 30px; }
        h2 { color: blue; font-size: 22px; }
        p  { color: black; font-size: 16px; }
        li { color: black; font-size: 16px; }
        a  { color: blue; font-size: 16px; }
    ";
    let mut css_kurallari = parse_css(varsayilan_css);

    // A) Harici CSS dosyalarını (<link rel="stylesheet" href="...">) indir ve ekle (en fazla 3 dosya)
    if url.host != "anka.local" {
        for css_href in extract_stylesheet_links(&dom_agaci).into_iter().take(3) {
            if let Ok(css_url) = url.resolve(&css_href)
                && let Ok(css_resp) = fetch(&css_url)
            {
                css_kurallari.extend(parse_css(&css_resp.body));
            }
        }
    }

    // B) Sayfa içi <style>...</style> kurallarını ekle
    let sayfa_css = extract_styles(&dom_agaci);
    if !sayfa_css.trim().is_empty() {
        css_kurallari.extend(parse_css(&sayfa_css));
    }

    // İSTASYON 5: anka-layout (Kutuların Ekrandaki X, Y Koordinatlarını ve Satır Kaydırmayı Hesaplama)
    let yerlesim_kutulari = compute_layout(&dom_agaci, &css_kurallari, viewport_width);

    // İSTASYON 6: anka-paint (RGB Çizim Listesini Üretme)
    let cizim_listesi = build_display_list(&yerlesim_kutulari);

    let protokol_etiketi = if url.scheme == "https" {
        "HTTPS/TLS"
    } else {
        "HTTP"
    };
    let durum = format!(
        "{} {} | {} bayt indirildi | {} satir/kutu Telek Engine v0.2.1 ile cizildi",
        protokol_etiketi,
        yanit.status_code,
        yanit.body.len(),
        cizim_listesi.len()
    );

    (sayfa_basligi, durum, cizim_listesi)
}

fn main() {
    let baslangic_adresi = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://anka.local/hosgeldiniz".to_string());

    // Windows'ta doğrudan gerçek yerel masaüstü penceresini aç!
    #[cfg(windows)]
    {
        win32_gui::pencereyi_baslat(&baslangic_adresi);
    }

    // Linux / macOS ortamında (ve GitHub Actions CI'da) terminal önizlemesi çalışır
    #[cfg(not(windows))]
    {
        let (baslik, durum, cizim_listesi) = motoru_calistir(&baslangic_adresi, 860);
        println!(
            "Anka Browser v0.2.1 ({}) [{}] -> {}",
            baslangic_adresi,
            baslik.unwrap_or_else(|| "Adsız".to_string()),
            durum
        );
        for komut in &cizim_listesi {
            println!(
                "[x:{:>2}, y:{:>3} | {:>2}px | RGB({:>3},{:>3},{:>3})] <{}> {}",
                komut.x,
                komut.y,
                komut.font_size,
                komut.color.r,
                komut.color.g,
                komut.color.b,
                komut.tag,
                komut.text
            );
        }
    }
}
