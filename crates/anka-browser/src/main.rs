// Anka Browser - Ana Tarayıcı Uygulaması (6 İstasyonlu Motor + Yerel Masaüstü Penceresi)

use anka_css::parse_css;
use anka_html::parse_html;
use anka_layout::compute_layout;
use anka_net::{fetch, parse_http_response};
use anka_paint::{PaintCommand, build_display_list};
use anka_url::Url;

#[cfg(windows)]
mod win32_gui;

/// 6 İSTASYONLUK MOTORU ATEŞLEYEN ORTAK FONKSİYON:
/// Verilen adresi alır: anka-url -> anka-net -> anka-html -> anka-css -> anka-layout -> anka-paint
/// sırasıyla işleyip (Durum Mesajı, Çizim Listesi) döndürür.
pub fn motoru_calistir(hedef_adres: &str) -> (String, Vec<PaintCommand>) {
    let hedef = hedef_adres.trim();

    // İSTASYON 1: anka-url (Adresi Doğrama)
    let url = match Url::parse(hedef) {
        Ok(u) => u,
        Err(hata) => {
            return (format!("Hata [anka-url] Gecersiz adres ({hedef}): {hata:?}"), Vec::new());
        }
    };

    // İSTASYON 2: anka-net (İnternetten veya Dahili Sayfadan HTML İndirme)
    let yanit = if url.host == "anka.local" {
        let dahili_http = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: text/html\r\n\r\n",
            "<html><body>",
            "<h1>Anka Browser'a Hos Geldiniz!</h1>",
            "<h2>Sifirdan Rust ile Yazilan Yerli Web Motoru (Telek Engine v0.1)</h2>",
            "<p>Su an hicbir hazir tarayici motoru veya agir GUI kutuphanesi olmadan, kendi yazdiginiz</p>",
            "<p>6 istasyonlu motorun gercek Windows masaustu penceresindesiniz!</p>",
            "<a>Yukaridaki 'CERN (1991)' butonuna tiklayarak dunyanin ilk web sitesini canli acabilirsiniz.</a>",
            "<a>Veya adres cubuguna http://example.com yazip 'Git ->' butonuna basabilirsiniz.</a>",
            "</body></html>"
        );
        match parse_http_response(dahili_http) {
            Ok(r) => r,
            Err(e) => return (format!("Hata [anka-net] Dahili yanit hatasi: {e:?}"), Vec::new()),
        }
    } else {
        match fetch(&url) {
            Ok(resp) => resp,
            Err(hata) => {
                return (format!("Hata [anka-net] Baglanti kurulamadi ({hedef}): {hata:?}"), Vec::new());
            }
        }
    };

    // İSTASYON 3: anka-html (HTML Metnini DOM Ağacına Çevirme)
    let dom_agaci = parse_html(&yanit.body);

    // İSTASYON 4: anka-css (Varsayılan Tarayıcı Stillerini Ayrıştırma)
    let varsayilan_css = "
        h1 { color: red; font-size: 30px; }
        h2 { color: blue; font-size: 22px; }
        p  { color: black; font-size: 16px; }
        a  { color: blue; font-size: 16px; }
    ";
    let css_kurallari = parse_css(varsayilan_css);

    // İSTASYON 5: anka-layout (Kutuların Ekrandaki X, Y Koordinatlarını Hesaplama)
    let yerlesim_kutulari = compute_layout(&dom_agaci, &css_kurallari, 860);

    // İSTASYON 6: anka-paint (RGB Çizim Listesini Üretme)
    let cizim_listesi = build_display_list(&yerlesim_kutulari);

    let durum = format!(
        "HTTP {} | {} bayt indirildi | {} kutu Telek Engine ile cizildi",
        yanit.status_code,
        yanit.body.len(),
        cizim_listesi.len()
    );

    (durum, cizim_listesi)
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
        let (durum, cizim_listesi) = motoru_calistir(&baslangic_adresi);
        println!("Anka Browser ({baslangic_adresi}) -> {durum}");
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
