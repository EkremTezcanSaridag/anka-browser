# 🦅 Anka Browser (Telek Engine) — Proje Hafızası ve Devir-Teslim Özeti (`CONTEXT.md`)

Bu doküman, geliştirme ortamı **Windows'tan Milis Linux'a** taşınırken Antigravity asistanının projeyi, mimari kararları ve çalışma disiplinini kaldığı yerden eksiksiz sürdürebilmesi için hazırlanmıştır.

---

## 1. Proje Kimliği ve Vizyonu
- **Proje Adı:** Anka Browser
- **Motor Adı:** Telek Engine (*Anka kuşunun uçmasını sağlayan ana kanat tüyü*)
- **GitHub Deposu:** `https://github.com/EkremTezcanSaridag/anka-browser`
- **Dil & Yapı:** Rust (`edition = "2024"`, Cargo Workspace / Monorepo, Lisans: `MPL-2.0`)
- **Mevcut Son Sürüm:** `v0.2.5` (29/29 birim testi başarılı, sıfır `clippy` uyarısı, sıfır üçüncü parti crate bağımlılığı)
- **Temel Felsefe:** Hazır bir Chromium/WebKit/Gecko fork'u veya WebView kabuğu **değildir**. URL ayrıştırmadan ağ katmanına, HTML/DOM ağacından CSS, Layout ve Paint aşamasına kadar tüm boru hattı (pipeline) sıfırdan, eğitici ve anlaşılır Türkçe yorum satırlarıyla yazılmaktadır.

---

## 2. Kullanıcı İle Çalışma Kuralları (Çok Önemli!)
1. **Öğreterek ve Açıklayarak İlerleme:** Kullanıcı projenin her satırına hakim olmak istemektedir. Yazılan her yeni fonksiyon veya kavram, ne işe yaradığı sade ve anlaşılır bir dille (ve kod içinde Türkçe yorum satırlarıyla) açıklanmalıdır.
2. **Sıfır Hata & Sıfır Uyarı Prensibi:** GitHub'a gönderilen her kod mutlaka şu 3 kontrolden hatasız geçmelidir:
   - `cargo check --workspace --all-targets`
   - `cargo test --workspace`
   - `cargo clippy --workspace --all-targets -- -D warnings`
3. **Git, PR ve Dal (Branch) Temizliği Disiplini:**
   - Yeni özellikler veya sürüm paketleri ayrı bir dalda (`feat/...`) geliştirilir ve GitHub'a PR olarak gönderilir.
   - Kullanıcı PR'ı inceleyip Merge ettikten sonra **işi biten dal hem yerelden (`git branch -D`) hem de GitHub'dan (`git push origin --delete`) mutlaka silinir**; repoda sadece temiz `main` dalı bırakılır.
   - Sürüm numaralandırmasında her küçük değişiklikte `0.3`, `0.4` diye atlanmaz; ara iyileştirmeler `v0.2.x` (`v0.2.3` vb.) olarak gider, yalnızca büyük dönüm noktaları (örn. Çoklu Sekme + Görseller + Linux GUI) tamamlandığında `v0.3.0` etiketi basılır.

---

## 3. Mevcut 6 İstasyonlu Mimari (`crates/`)

| Modül | Mevcut Yetenekler (`v0.2.2` İtibarıyla) | Test Sayısı |
| :--- | :--- | :---: |
| **`crates/anka-url`** | `Url::parse` (scheme, host, port, path, query, fragment), `effective_port` (`http -> 80`, `https -> 443`), `Url::resolve` (bağıl URL çözümleme). | 4 ✅ |
| **`crates/anka-net`** | `HTTP/1.1` TCP istemcisi (`fetch`), ham ikili bayt (`raw_body: Vec<u8>`) ve `parse_http_response_bytes`, `301/302/303/307/308` otomatik yönlendirme takibi (`MAX_REDIRECTS = 5`), bayt düzeyinde `Transfer-Encoding: chunked` birleştirici (`decode_chunked_bytes`), büyük/küçük harf duyarsız `get_header`. Windows'ta `winhttp_tls.rs`, tüm Linux dağıtımlarında `linux_tls.rs` (OpenSSL FFI) ile sıfır bağımlılıklı `HTTPS/TLS 1.3`. | 5 ✅ |
| **`crates/anka-html`** | Hata toleranslı (kapanmamış etiketlerde çökmeyen) HTML5 Tokenizer ve DOM ağacı (`DomNode`), etiket özellikleri (`attributes`), `extract_title` (`<title>`), `extract_styles` (`<style>`), `extract_stylesheet_links` (`<link rel="stylesheet">`), `decode_html_entities` (`&amp;`, `&nbsp;` vb.). | 6 ✅ |
| **`crates/anka-css`** | `parse_css`: Seçici (`selector`) ve özellik (`Declaration`: `property`, `value`) ayrıştırıcı. | 2 ✅ |
| **`crates/anka-layout`** | `compute_layout`: DOM + CSS birleştirme, `selector_matches` (`tag`, `.class`, `#id`, `tag.class`), satır içi `style="..."`, otomatik satır kaydırma (`wrap_text`), `<li>` madde işareti (`• `), `<hr>` yatay çizgi, `<img src alt width height>` görsel kutu yerleşimi (`image_src`), kısa `<a>`/`<span>`/`<button>` öğelerinin yan yana (`inline`) dizilimi, `bg_color` ve `border_color`. | 4 ✅ |
| **`crates/anka-paint`** | `build_display_list`: İngilizce renk adlarını ve 6/3 haneli Hex renk kodlarını (`#ff5733`, `#0f0`) `RgbColor` piksellerine ve `PaintCommand` listesine dönüştürme. Sıfır crate ile `DecodedImage` görsel çözücü (saf Rust BMP & PPM P6 + Linux `gdk-pixbuf` PNG/JPEG/GIF/WebP + dahili Anka amblemi üreticisi). | 4 ✅ |
| **`crates/anka-browser`** | `motoru_calistir`, akıllı adres çubuğu (`normalize_address`), çoklu sekme (`Vec<TabState>`) ve yer imleri (`Vec<Bookmark>`, `anka.local/yer-imleri`) destekli ortak tarayıcı durumu (`browser_state.rs`), Windows için Win32 GDI masaüstü penceresi (`win32_gui.rs`), Linux (Wayland/X11) için GTK3 + Cairo 2D masaüstü penceresi (`linux_gui.rs`) ve CI için terminal önizlemesi (`main.rs`). | 4 ✅ |

---

## 4. Milis Linux'ta Sıradaki İlk Hedefler (`v0.2.3` / `v0.3.0`)

Windows'ta kullandığımız `win32_gui.rs` ve `winhttp_tls.rs` dosyaları `#[cfg(windows)]` ile izole edilmiştir. **Milis Linux** üzerinde geliştirirken ilk yapılacaklar:

1. **🐧 Milis Linux Yerel Grafik Penceresi (`#[cfg(target_os = "linux")]`):**
   - Şu an Linux'ta `cargo run` çalıştırıldığında `#[cfg(not(windows))]` bloğu terminal önizlemesi basmaktadır.
   - Milis Linux üzerinde gerçek masaüstü penceresi açmak için hafif bir Linux pencere/çizim katmanı (X11 / Wayland veya hafif Rust pencere sistemi) eklenecektir.
2. **🔒 Linux `HTTPS / TLS 1.3` Desteği (`anka-net`):**
   - `anka-net` içinde `#[cfg(not(windows))]` altında Linux için HTTPS/TLS bağlantısı (`rustls` veya sistem TLS soketi) aktif edilecektir.
3. **📑 `v0.3.0` Büyük Hedefleri:**
   - Çoklu Sekme (Multi-Tab) yönetimi, Yer İmleri (Bookmarks) ve `<img src="...">` görsel çizimi.
