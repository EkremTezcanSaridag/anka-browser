// Anka Browser - Ana Tarayıcı Uygulaması (6 İstasyonlu Motor + Yerel Masaüstü Penceresi)

use anka_css::parse_css;
use anka_html::{extract_styles, extract_stylesheet_links, extract_title, parse_html};
use anka_layout::compute_layout;
use anka_net::{fetch, parse_http_response};
use anka_paint::{DecodedImage, PaintCommand, build_display_list};
use anka_url::Url;

#[cfg(any(windows, target_os = "linux", test))]
mod browser_state;

#[cfg(windows)]
mod win32_gui;

#[cfg(target_os = "linux")]
mod linux_gui;

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

/// verilen ham HTML/HTTP yanıtını 6 istasyondan geçirip çizer (örn: `http://anka.local/yer-imleri`).
pub fn html_motoru_calistir(
    sayfa_url_str: &str,
    ham_http_yaniti: &str,
    viewport_width: u32,
) -> (Option<String>, String, Vec<PaintCommand>) {
    let Ok(url) = Url::parse(sayfa_url_str) else {
        return (None, "Gecersiz dahili URL".to_string(), Vec::new());
    };
    let Ok(yanit) = parse_http_response(ham_http_yaniti) else {
        return (None, "Dahili HTTP yanit hatasi".to_string(), Vec::new());
    };
    yaniti_cizime_donustur(&url, &yanit, viewport_width)
}

fn yaniti_cizime_donustur(
    url: &Url,
    yanit: &anka_net::HttpResponse,
    viewport_width: u32,
) -> (Option<String>, String, Vec<PaintCommand>) {
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

    // İSTASYON 6: anka-paint (RGB Çizim Listesini Üretme ve `<img>` Görsellerini Çözme)
    let mut cizim_listesi = build_display_list(&yerlesim_kutulari);

    // Sayfadaki `<img>` etiketleri için görselleri indirip piksellerine (`DecodedImage`) çöz (en fazla 6 görsel):
    let mut indirilen_gorsel = 0usize;
    for komut in &mut cizim_listesi {
        if let Some(src) = &komut.image_src {
            // A) Dahili Anka test görseli (`http://anka.local/anka-logo.bmp` veya `/anka-logo.bmp`)
            if src.contains("anka-logo") {
                komut.image = Some(DecodedImage::generate_anka_banner(
                    komut.width,
                    komut.height,
                ));
                continue;
            }

            // B) Gerçek internet üzerindeki görseller (`https://...` veya bağıl `/resim.png`)
            if indirilen_gorsel < 6
                && url.host != "anka.local"
                && let Ok(img_url) = url.resolve(src)
                && let Ok(img_resp) = fetch(&img_url)
                && let Some(decoded) = DecodedImage::decode_bytes(&img_resp.raw_body)
            {
                komut.image = Some(decoded);
                indirilen_gorsel += 1;
            }
        }
    }

    let protokol_etiketi = if url.scheme == "https" {
        "HTTPS/TLS"
    } else {
        "HTTP"
    };
    let durum = format!(
        "{} {} | {} bayt indirildi | {} satir/kutu Telek Engine v0.2.5 ile cizildi",
        protokol_etiketi,
        yanit.status_code,
        yanit.body.len(),
        cizim_listesi.len()
    );

    (sayfa_basligi, durum, cizim_listesi)
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
            "h1 { color: #dc2626; background-color: #fef2f2; border: 1px solid #fecaca; font-size: 30px; }",
            "h2 { color: #1d4ed8; font-size: 20px; }",
            "p  { color: #334155; font-size: 16px; }",
            "li { color: #0f766e; font-size: 16px; }",
            ".pill { color: #1e40af; background-color: #dbeafe; border: 1px solid #93c5fd; font-size: 15px; }",
            ".kart { color: #065f46; background-color: #ecfdf5; border: 1px solid #6ee7b7; font-size: 16px; }",
            "#duyuru { color: #9a3412; background-color: #ffedd5; border: 1px solid #fdba74; font-size: 16px; }",
            "</style></head><body>",
            "<h1>Anka Browser v0.2.5 — Telek Engine</h1>",
            "<nav>",
            "<a class=\"pill\" href=\"https://example.com\">HTTPS Example.com</a>",
            "<a class=\"pill\" href=\"http://info.cern.ch\">CERN (1991)</a>",
            "<a class=\"pill\" href=\"http://anka.local/yer-imleri\">Yer Imlerim</a>",
            "<a class=\"pill\" href=\"https://tr.wikipedia.org\">Vikipedi TR</a>",
            "</nav>",
            "<hr>",
            "<img src=\"http://anka.local/anka-logo.bmp\" alt=\"Anka Telek Engine Piksel Amblemi\" width=\"380\" height=\"110\">",
            "<h2>Gercek Gorsel (&lt;img&gt;) Cizimi, Coklu Sekme ve Yer Imleri!</h2>",
            "<div class=\"kart\"><p>Yukaridaki gorsel Telek Engine'in sifir bagimlilikli piksel motoruyla (Linux Cairo &amp; Windows GDI) gercek RGB pikselleri olarak cizilmistir.</p></div>",
            "<div id=\"duyuru\"><p>BMP ve PPM gorselleri saf Rust cozucumuzle, Linux'ta PNG/JPEG/GIF/WebP gorselleri ise sistem gdk-pixbuf motoruyla cozulur.</p></div>",
            "<ul>",
            "<li>&lt;img src=&quot;...&quot; alt=&quot;...&quot; width=&quot;...&quot; height=&quot;...&quot;&gt; kutu yerlesimi ve gercek piksel cizimi</li>",
            "<li>Coklu Sekme (Vec&lt;TabState&gt;) ve Yer Imleri (anka.local/yer-imleri)</li>",
            "<li>Sifir harici crate bagimliligiyla Linux (GTK3/Cairo) ve Windows (Win32/GDI)</li>",
            "</ul>",
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

    yaniti_cizime_donustur(&url, &yanit, viewport_width)
}

fn main() {
    let baslangic_adresi = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://anka.local/hosgeldiniz".to_string());

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

    // Ekransız (Headless / CI) ortamda veya diğer platformlarda terminal önizlemesi çalışır
    #[cfg(not(windows))]
    {
        let (baslik, durum, cizim_listesi) = motoru_calistir(&baslangic_adresi, 860);
        println!(
            "Anka Browser v0.2.5 ({}) [{}] -> {}",
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
