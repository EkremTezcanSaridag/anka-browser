# 🦅 Anka Browser (Telek Engine) — Proje Hafızası ve Devir-Teslim Özeti (`CONTEXT.md`)

Bu doküman, geliştirme ortamı **Windows'tan Milis Linux'a** taşınırken Antigravity asistanının projeyi, mimari kararları ve çalışma disiplinini kaldığı yerden eksiksiz sürdürebilmesi için hazırlanmıştır.

---

## 1. Proje Kimliği ve Vizyonu
- **Proje Adı:** Anka Browser
- **Motor Adı:** Telek Engine (*Anka kuşunun uçmasını sağlayan ana kanat tüyü*)
- **GitHub Deposu:** `https://github.com/EkremTezcanSaridag/anka-browser`
- **Dil & Yapı:** Rust (`edition = "2024"`, Cargo Workspace / Monorepo, Lisans: `MPL-2.0`)
- **Mevcut Son Sürüm:** `v0.3.4` (70/70 birim testi başarılı, sıfır `clippy` uyarısı, sıfır üçüncü parti crate ve sıfır Linux `-dev` paketi bağımlılığı)
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

| Modül | Mevcut Yetenekler (`v0.3.4` İtibarıyla) | Test Sayısı |
| :--- | :--- | :---: |
| **`crates/anka-url`** | `Url::parse` (scheme, host, port, path, query, fragment), `effective_port` (`http -> 80`, `https -> 443`), `file://` yerel dosya URL'leri ve `Url::resolve` (bağıl URL çözümleme). | 5 ✅ |
| **`crates/anka-net`** | `HTTP/1.1` TCP istemcisi, `fetch_request` (`GET` + `POST` `application/x-www-form-urlencoded`, **`CookieJar`** ile `Set-Cookie`/`Cookie:`, `Domain`/`Path`/`Secure`/`Expires` HTTP tarih ayrıştırıcı/`Max-Age`/kalıcı TSV disk serileme), `fetch_with_shield` + **`shield` (Anka Kalkan)**, yerel `file://`, saf Rust `GZIP / DEFLATE`, `301-308` yönlendirme (`POST->GET` + redirect çerez aktarımı), Windows WinHTTP (`WINHTTP_QUERY_SET_COOKIE`) + Linux `dlopen` TLS 1.3. | 14 ✅ |
| **`crates/anka-html`** | Hata toleranslı HTML5 Tokenizer ve DOM ağacı (`DomNode`), `find_by_selector_mut` (`#id`/`.class`/`tag`), özellik değerlerinde entity çözümü (`&amp;`, sayısal `&#...`), boolean özellikler (`hidden`, `disabled`), çok satırlı yorumlar, ham `<style>`/`<script>`/`<textarea>` metin okuma, `extract_title`, `extract_styles`, `extract_stylesheet_links`. | 8 ✅ |
| **`crates/anka-css`** | `parse_css`: `/* ... */` CSS yorumlarını temizleyen, parantez derinliği takipli `@media`/`@keyframes`/`@font-face` bloklarını ve `@import`/`@charset` kurallarını güvenle atlayan seçici ve `Declaration` ayrıştırıcı. | 3 ✅ |
| **`crates/anka-layout`** | `compute_layout_with_images` & `resolve_body_background`: soy/çocuk CSS seçici birleştiricileri, `:link`/`:visited`, `width`/`max-width`, `border-radius`, **`box-shadow`**, **`font-weight`/`font-style`** kalıtımı, `display: none` / `hidden` / `<noscript>`/`<datalist>`/`<option>` atlama, `<br>`, `<table>`/`<tr>`, `<select>`, doğal en-boy oranlı `<img>`, inline akış, flexbox, `<form method="post">`, `<input type="password">` ve çok satırlı `<textarea>` (`is_multiline`). | 15 ✅ |
| **`crates/anka-paint`** | `build_display_list` & `RgbColor::try_from_css`: `transparent`/`none`/`rgba(..., 0)` saydamlık, `var(--..., fallback)`, `!important`, `rgb()`/`rgba()`, Hex/renk adı, `border_radius`/`has_shadow`/`is_bold`/`is_italic`/`is_multiline`/`is_password`/`is_post_form` çizim alanları; saf Rust BMP & PPM P6 + Linux `dlopen` `gdk-pixbuf` PNG/JPEG/GIF/WebP çözümü. | 4 ✅ |
| **`crates/anka-js`** | **7. İstasyon (Sıfırdan JS Motoru):** Kendi Lexer ve Evaluator'ı (`JsRuntime`), `let/const/var`, aritmetik/metin birleştirme, `if/else`, `function` + anonim callback, `console.log`, `document.title`, `document.getElementById` & **`document.querySelector`**, **`addEventListener("click", ...)`**, `setAttribute`, `innerText` ve `style.*`. | 4 ✅ |
| **`crates/anka-browser`** | **Çok Süreçli (Multi-Process) Mimari** (`process_ipc.rs`), diskte kalıcı **`history.tsv`** (`anka.local/gecmis` arama/temizleme + **`Ctrl+H`**) ve **`cookies.tsv`**, `POST` formlar & çok satırlı **`<textarea>`** (`anka.local/not`), maskeli parola (`•`), **DevTools (`anka.local/gelistirici`)**, **`view-source:` (`Ctrl+U`)**, sekme bazlı **Yakınlaştırma (`Ctrl+`/`-`/`0`, `%50..%200`)** + ölçekli tıklama, **`Ctrl+F` Sayfa İçi Arama** (canlı sayac, sarı/turuncu kutu vurgusu ve otomatik kaydırma), `Tab`/`Shift+Tab`/`Ctrl+T/W/L/R/F5`, **`border-radius` & `box-shadow`** Cairo/Win32 çizimi. | 17 ✅ |

---

## 4. Sıradaki İsteğe Bağlı Geliştirme Alanları (`v0.3.4+`)

CSS `box-shadow` & `font-weight`/`font-style`, sekme bazlı yakınlaştırma (`Ctrl+`/`-`/`0`) ve `Ctrl+F` sayfa içi arama (eşleşme vurgusu & otomatik kaydırma) tamamlanmıştır! Sıradaki doğal adımlar:

1. **🎨 Gelişmiş CSS Düzeni:** `position: relative/absolute`, `z-index` katmanları, `line-height` & CSS Grid iyileştirmeleri.
2. **⚡ Asenkron JS & Ağ API:** `setTimeout`, `fetch()`, DOM öğe oluşturma (`createElement`/`appendChild`) ve form `<input>` `value` okuma/güncelleme.
3. **🧭 Tarayıcı Konforu:** Kapatılan sekmeyi geri açma (`Ctrl+Shift+T`), yer imi düzenleme/dışa aktarma ve dosya indirme yöneticisi.
