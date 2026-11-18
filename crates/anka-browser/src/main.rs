// Anka Browser - Ana Tarayıcı Uygulaması (İstasyon 6B - Tüm Motorun Birleşimi)

use anka_css::parse_css;
use anka_html::parse_html;
use anka_layout::compute_layout;
use anka_net::{fetch, parse_http_response};
use anka_paint::build_display_list;
use anka_url::Url;

fn main() {
    // Kullanıcı komut satırından bir adres verdiyse onu al (örn: `cargo run -- http://example.com`),
    // vermediyse dahili Anka başlangıç sayfasını aç!
    let hedef_adres = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://anka.local/hosgeldiniz?surum=0.1#baslangic".to_string());

    println!("======================================================================");
    println!(" 🦅 ANKA BROWSER MOTORU (Telek Engine v0.1) BAŞLATILDI");
    println!("======================================================================\n");

    // -------------------------------------------------------------------------
    // İSTASYON 1: anka-url (Adresi Doğrama)
    // -------------------------------------------------------------------------
    let url = match Url::parse(&hedef_adres) {
        Ok(u) => u,
        Err(hata) => {
            eprintln!("[HATA - anka-url] Geçersiz URL ({hedef_adres}): {hata:?}");
            return;
        }
    };

    println!("[1/6 anka-url] Adres Başarıyla Çözümlendi:");
    println!(
        "     Protokol: {} | Sunucu: {} | Kapı (Port): {:?} | Yol: {}\n",
        url.scheme,
        url.host,
        url.effective_port(),
        url.path
    );

    // -------------------------------------------------------------------------
    // İSTASYON 2: anka-net (HTTP Yanıtını İndirme ve Ayıklama)
    // -------------------------------------------------------------------------
    let yanit = if url.host == "anka.local" {
        // Dahili test sayfası (İnternet olmadan da tüm motoru test edebilmek için)
        let dahili_http = concat!(
            "HTTP/1.1 200 OK\r\n",
            "Content-Type: text/html\r\n",
            "Server: AnkaLocal\r\n\r\n",
            "<html>",
            "<body>",
            "<h1>Anka Browser'a Hos Geldiniz!</h1>",
            "<h2>Sifirdan Rust ile Yazilan Yerli Web Motoru</h2>",
            "<p>Bu sayfa 6 istasyonluk Telek Engine boru hattindan gecerek cizildi.</p>",
            "<p>Canli bir site denemek icin: cargo run -- http://example.com</p>",
            "</body>",
            "</html>"
        );
        parse_http_response(dahili_http).expect("Dahili HTTP yanıtı ayrıştırılamadı")
    } else {
        println!("[2/6 anka-net] {}:{} sunucusuna bağlanılıyor...", url.host, url.effective_port().unwrap_or(80));
        match fetch(&url) {
            Ok(resp) => resp,
            Err(hata) => {
                eprintln!("[HATA - anka-net] Sunucuya bağlanılamadı: {hata:?}");
                return;
            }
        }
    };

    println!(
        "[2/6 anka-net] HTTP Yanıtı Alındı! Durum Kodu: {} | Boyut: {} bayt\n",
        yanit.status_code,
        yanit.body.len()
    );

    // -------------------------------------------------------------------------
    // İSTASYON 3: anka-html (HTML Metnini DOM Ağacına Çevirme)
    // -------------------------------------------------------------------------
    let dom_agaci = parse_html(&yanit.body);
    println!("[3/6 anka-html] DOM Ağacı Kuruldu! Kök Düğüm: {:?}\n", dom_agaci.node_type);

    // -------------------------------------------------------------------------
    // İSTASYON 4: anka-css (Varsayılan Tarayıcı Stillerini Ayrıştırma)
    // -------------------------------------------------------------------------
    let varsayilan_css = "
        h1 { color: red; font-size: 32px; }
        h2 { color: blue; font-size: 22px; }
        p  { color: green; font-size: 16px; }
    ";
    let css_kurallari = parse_css(varsayilan_css);
    println!(
        "[4/6 anka-css] {} adet CSS Stil Kuralı Yüklendi.\n",
        css_kurallari.len()
    );

    // -------------------------------------------------------------------------
    // İSTASYON 5: anka-layout (Kutuların Ekrandaki X, Y Koordinatlarını Hesaplama)
    // -------------------------------------------------------------------------
    let pencere_genisligi = 800;
    let yerlesim_kutulari = compute_layout(&dom_agaci, &css_kurallari, pencere_genisligi);
    println!(
        "[5/6 anka-layout] {} adet kutunun (x, y, genişlik, yükseklik) koordinatı hesaplandı.\n",
        yerlesim_kutulari.len()
    );

    // -------------------------------------------------------------------------
    // İSTASYON 6: anka-paint (RGB Renkli Çizim Listesini Üretme ve Ekrana Çizme)
    // -------------------------------------------------------------------------
    let cizim_listesi = build_display_list(&yerlesim_kutulari);
    println!("[6/6 anka-paint] Çizim Listesi (Display List) ve RGB Pikseller Hazır!\n");

    println!("┌────────────────────────────────────────────────────────────────────┐");
    println!("│ 🌐 Adres Çubuğu: {:<50} │", hedef_adres);
    println!("├────────────────────────────────────────────────────────────────────┤");
    for komut in &cizim_listesi {
        println!(
            "│ [x:{:>2}, y:{:>3} | {:>2}px | RGB({:>3},{:>3},{:>3})] <{}> {}",
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
    println!("└────────────────────────────────────────────────────────────────────┘");
}
