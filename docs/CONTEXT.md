# 🦅 Anka Browser (Telek Engine) — Proje Hafızası ve Devir-Teslim Özeti (`CONTEXT.md`)

Bu doküman, geliştirme ortamı **Windows'tan Milis Linux'a** taşınırken Antigravity asistanının projeyi, mimari kararları ve çalışma disiplinini kaldığı yerden eksiksiz sürdürebilmesi için hazırlanmıştır.

---

## 1. Proje Kimliği ve Vizyonu
- **Proje Adı:** Anka Browser
- **Motor Adı:** Telek Engine (*Anka kuşunun uçmasını sağlayan ana kanat tüyü*)
- **GitHub Deposu:** `https://github.com/EkremTezcanSaridag/anka-browser`
- **Dil & Yapı:** Rust (`edition = "2024"`, Cargo Workspace / Monorepo, Lisans: `MPL-2.0`)
- **Mevcut Son Sürüm:** `v0.2.9` (43/43 birim testi başarılı, sıfır `clippy` uyarısı, sıfır üçüncü parti crate ve sıfır Linux `-dev` paketi bağımlılığı)
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

## 3. Mevcut 7 İstasyonlu Mimari (`crates/`)

| Modül | Mevcut Yetenekler (`v0.2.9` İtibarıyla) | Test Sayısı |
| :--- | :--- | :---: |
| **`crates/anka-url`** | `Url::parse` (scheme, host, port, path, query, fragment), `effective_port` (`http -> 80`, `https -> 443`), `file://` yerel dosya URL'leri ve `Url::resolve` (bağıl URL çözümleme). | 5 ✅ |
| **`crates/anka-net`** | `HTTP/1.1` TCP istemcisi (`fetch`), yerel `file://` protokolü, ham ikili bayt (`raw_body: Vec<u8>`), saf Rust `GZIP / DEFLATE` açıcısı (`gzip.rs`), `301/302/303/307/308` yönlendirme takibi, bayt düzeyinde `chunked` birleştirici. Windows'ta `winhttp_tls.rs`, tüm Linux'larda `dlopen` `linux_tls.rs` ile `HTTPS/TLS 1.3`. | 6 ✅ |
| **`crates/anka-html`** | Hata toleranslı HTML5 Tokenizer ve DOM ağacı (`DomNode`), değer-siz boolean özellikler (`hidden`, `disabled`), çok satırlı `<!-- ... -->` yorumları, ham `<style>` metin okuma (`div > p` uyumlu), sayısal (`&#123;`, `&#x21;`) + genişletilmiş adlandırılmış HTML entity çözümü, `extract_title`, `extract_styles`, `extract_stylesheet_links`. | 8 ✅ |
| **`crates/anka-css`** | `parse_css`: `/* ... */` CSS yorumlarını temizleyen, parantez derinliği takipli `@media`/`@keyframes`/`@font-face` bloklarını ve `@import`/`@charset` kurallarını güvenle atlayan seçici ve `Declaration` ayrıştırıcı. | 3 ✅ |
| **`crates/anka-layout`** | `compute_layout_with_images` & `resolve_body_background`: DOM + CSS birleştirme, `display: none` / `hidden` / `<noscript>` atlama, `<br>` satır atlama, `<table>`/`<tr>` yatay tablo satırı yerleşimi, doğal en-boy oranlı `<img>` ölçümü, `<strong>`/`<b>`/`<em>`/`<i>`/`<code>`/`<small>` inline akışı, flexbox (`display: flex`, `gap`), `<form>`/`<input>`. | 9 ✅ |
| **`crates/anka-paint`** | `build_display_list` & `RgbColor::try_from_css`: `transparent`/`none`/`rgba(..., 0)` saydamlık yönetimi, `rgb(...)`/`rgba(...)`, 3/6/8 haneli Hex ve 30+ CSS renk adı; saf Rust BMP & PPM P6 + Linux `dlopen` `gdk-pixbuf` PNG/JPEG/GIF/WebP görsel çözümü. | 4 ✅ |
| **`crates/anka-js`** | **7. İstasyon (Sıfırdan JS Motoru):** Kendi Lexer ve Evaluator'ı (`JsRuntime`), `let/const/var`, aritmetik/metin birleştirme, `if/else`, `function`, `console.log`, `document.title` ve canlı DOM güncellemesi (`document.getElementById(...).innerText` ve `style.*`). | 3 ✅ |
| **`crates/anka-browser`** | `motoru_calistir` & canlı `LoadedPage::run_inline_js`, `<body>` sayfa arka plan rengi (`page_bg`), kalın (`strong`/`b`) ve italik (`em`/`i`) çizim, çoklu sekme (`Vec<TabState>`), kalıcı yer imleri (`Vec<Bookmark>`), HTML formları, Windows Win32 GDI ve Linux `dlopen` GTK3/Cairo 2D GUI. | 5 ✅ |

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
