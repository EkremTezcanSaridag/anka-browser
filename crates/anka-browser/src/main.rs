// Anka Browser - Ana Tarayıcı Uygulaması (6 İstasyonlu Motor + Yerel Masaüstü Penceresi)

use anka_css::parse_css;
use anka_html::{extract_styles, extract_title, parse_html};
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
pub fn motoru_calistir(hedef_adres: &str) -> (Option<String>, String, Vec<PaintCommand>) {
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
            "h1 { color: #dc2626; font-size: 32px; }",
            "h2 { color: #1d4ed8; font-size: 20px; }",
            "p  { color: #334155; font-size: 16px; }",
            "</style></head><body>",
            "<h1>Anka Browser v0.2 — Telek Engine</h1>",
            "<h2>Artik HTTPS / TLS 1.3, Chunked HTTP ve Akilli Adres Cubugu Hazir!</h2>",
            "<p>Su an hicbir hazir tarayici motoru veya dis kutuphane olmadan kendi yazdiginiz</p>",
            "<p>yerli web motorunun gercek Windows masaustu penceresindesiniz.</p>",
            "<a>Yukaridaki 'HTTPS Example' butonuna tiklayarak sifreli HTTPS baglantisini test edebilirsiniz!</a>",
            "<a>Veya adres cubuguna sadece example.com yazip 'Git ->' butonuna basabilirsiniz.</a>",
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

    // İSTASYON 4: anka-css (Varsayılan Tarayıcı Stilleri + Sayfanın Kendi `<style>` Kuralları)
    let varsayilan_css = "
        h1 { color: red; font-size: 30px; }
        h2 { color: blue; font-size: 22px; }
        p  { color: black; font-size: 16px; }
        a  { color: blue; font-size: 16px; }
    ";
    let mut css_kurallari = parse_css(varsayilan_css);
    let sayfa_css = extract_styles(&dom_agaci);
    if !sayfa_css.trim().is_empty() {
        css_kurallari.extend(parse_css(&sayfa_css));
    }

    // İSTASYON 5: anka-layout (Kutuların Ekrandaki X, Y Koordinatlarını Hesaplama)
    let yerlesim_kutulari = compute_layout(&dom_agaci, &css_kurallari, 860);

    // İSTASYON 6: anka-paint (RGB Çizim Listesini Üretme)
    let cizim_listesi = build_display_list(&yerlesim_kutulari);

    let protokol_etiketi = if url.scheme == "https" {
        "HTTPS/TLS"
    } else {
        "HTTP"
    };
    let durum = format!(
        "{} {} | {} bayt indirildi | {} kutu Telek Engine v0.2 ile cizildi",
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
        let (baslik, durum, cizim_listesi) = motoru_calistir(&baslangic_adresi);
        println!(
            "Anka Browser v0.2 ({}) [{}] -> {}",
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
