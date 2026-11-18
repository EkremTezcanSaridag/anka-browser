# 🦅 Anka Browser

**Anka**, sıfırdan kendi web motoruyla (*Telek Engine*) geliştirilen, bağımsız ve açık kaynak kodlu bir web tarayıcısı projesidir.

Modern web dünyası bugün yalnızca birkaç dev motorun (Blink/Chromium, WebKit ve Gecko) tekeline sıkışmış durumdadır. **Anka**, bir web sayfasının ağ üzerinden indirilmesinden ekranda piksellere dönüştürülmesine kadar geçen tüm süreci bağımsız, modüler ve anlaşılır bir mimariyle sıfırdan inşa etmeyi hedefler.

---

## 🎯 Anka Nedir, Ne Değildir?

### ✅ Anka Nedir?
- **Sıfırdan Yazılan Bağımsız Bir Motordur:** HTML ayrıştırma (parsing), DOM ağacı, CSS stil hesaplama, sayfa yerleşimi (layout) ve ekrana çizim (painting) katmanlarını kendi bünyesinde sıfırdan geliştirir.
- **Modüler Bir Açık Kaynak Projesidir:** Her alt bileşen (`url`, `net`, `html`, `css`, `layout`, `paint`, `js`) birbirinden bağımsız test edilebilir kütüphaneler olarak tasarlanmıştır. Bu sayede topluluktan her seviyede geliştirici tek bir modüle odaklanarak kolayca katkıda bulunabilir.
- **Standart Odaklıdır:** WHATWG (HTML, URL, DOM), W3C (CSS) ve IETF (HTTP, TLS) spesifikasyonlarını doğrudan referans alır.
- **Gizlilik ve Performans Önceliklidir:** Gereksiz şişkinlikten (bloatware) uzak, telemetri içermeyen, hızlı ve hafif bir mimari hedefler.

### ❌ Anka Ne Değildir?
- **Bir Chromium, Firefox (Gecko) veya WebKit Fork'u Değildir:** Mevcut dev tarayıcıların kaynak kodunu kopyalayıp sadece logosunu veya arayüzünü değiştiren bir proje (re-skin / wrapper) **değildir**.
- **WebView / Electron Kabuğu Değildir:** İşletim sisteminin hazır tarayıcı bileşenini (`WebView2`, `CEF`, `WebKitGTK`) çağırıp sayfayı ona çizdirmez; kendi render boru hattını (rendering pipeline) kurar.
- **Kısa Vadeli Bir Oyuncak Değildir:** İlk günden tüm modern web'i kusursuz açması beklenemez; ancak adım adım büyüyen, uzun soluklu ve gerçek web standartlarını hedefleyen ciddi bir mühendislik yolculuğudur.

---

## 🏗️ Mimari ve Modül Yapısı

Anka'nın çekirdek motoru bir web sayfasını aşağıdaki boru hattı (pipeline) üzerinden işler:

```text
[URL] ──► (anka-url) ──► (anka-net: HTTP/TLS)
                               │
                               ▼
                         [Ham HTML & CSS]
                        ╱                ╲
                       ▼                  ▼
                 (anka-html)         (anka-css)
                  [DOM Ağacı]         [CSSOM]
                        ╲                ╱
                         ▼              ▼
                      [Stil Ağacı (Styled Tree)]
                               │
                               ▼
                         (anka-layout)
                      [Kutu Modeli & Dizilim]
                               │
                               ▼
                         (anka-paint)
                      [2D/GPU Piksel Çizimi]
                               │
                               ▼
                        (anka-browser)
                      [Pencere & Sekme UI]
```

| Modül | Görevi ve Kapsamı |
| :--- | :--- |
| **`anka-url`** | WHATWG standartlarına uygun URL ayrıştırıcı (`scheme://host:port/path?query#fragment`), bağıl URL çözümleme ve Punycode/IDNA desteği. |
| **`anka-net`** | TCP soket yönetimi, TLS 1.3 güvenli bağlantı, HTTP/1.1 & HTTP/2 ayrıştırıcı, bağlantı havuzu (`keep-alive`), `gzip`/`brotli` açma, Önbellek ve Çerez (Cookie Jar) yönetimi. |
| **`anka-html`** | Durum makinesi (State Machine) tabanlı HTML5 Tokenizer ve hata toleranslı DOM (Document Object Model) ağacı üreticisi. |
| **`anka-css`** | CSS Tokenizer, seçici eşleştirme (Selector Matching), özgüllük (Specificity) ve Cascade kuralları. |
| **`anka-layout`** | Kutu modeli (Margin, Border, Padding, Content), Block/Inline akışı, Flexbox ve metin ölçümleme. |
| **`anka-paint`** | Yerleşim ağacını çizim komutlarına (Display List) dönüştürme ve 2D/GPU rasterizasyon. |
| **`anka-js`** | JavaScript çalışma zamanı, Event Loop ve DOM API (`document.querySelector`, `addEventListener`) bağlamaları. |
| **`anka-browser`** | Çoklu sekme (tab) yönetimi, adres çubuğu, yer imleri, geçmiş ve kullanıcı arayüzü. |

---

## 🗺️ Yol Haritası (Roadmap)

- [ ] **Faz 0 — Temel İskelet ve Monorepo Kurulumu**
  - [x] Proje vizyonunun ve mimarisinin belirlenmesi (`README.md`)
  - [x] Rust Cargo çalışma alanı (Workspace) kurulumu
  - [ ] CI/CD (GitHub Actions), `CONTRIBUTING.md` ve açık kaynak lisansının eklenmesi
- [x] **Faz 1 — `anka-url` ve `anka-net` (Temel Ağ Katmanı)**
  - [x] WHATWG uyumlu URL ayrıştırıcı, varsayılan port çözümleyici ve birim testleri (`crates/anka-url`)
  - [x] HTTP/1.1 TCP istemcisi (`GET`, Headers, yanıt ayrıştırıcı) ve birim testleri (`crates/anka-net`)
  - [ ] İleri ağ özellikleri (TLS/HTTPS, Yönlendirme takibi, Chunked Transfer, Gzip)
- [x] **Faz 2 — `anka-html` ve `anka-css` (Ayrıştırma Katmanı)**
  - [x] HTML5 Tokenizer ve hata toleranslı DOM ağacı oluşturucu (`crates/anka-html`)
  - [x] CSS kural ayrıştırıcı ve stil bildirimleri (`crates/anka-css`)
- [x] **Faz 3 — `anka-layout` ve `anka-paint` ("Merhaba Web")**
  - [x] Block & Inline kutu modeli yerleşimi ve CSS stil eşleştirme (`crates/anka-layout`)
  - [x] RGB çizim komutları listesi (`crates/anka-paint`) ve 6 modülü birleştiren ana uygulama (`crates/anka-browser`)
- [ ] **Faz 4 — Tarayıcı Arayüzü ve Etkileşim**
  - [ ] Adres çubuğu, tıklanabilir bağlantılar (`<a href>`), kaydırma (scroll) ve çoklu sekme
  - [ ] Görsel (`<img>`) desteği ve Flexbox yerleşimi
- [ ] **Faz 5 — JavaScript, Süreç İzolasyonu ve Güvenlik**
  - [ ] JavaScript motoru ve DOM API entegrasyonu
  - [ ] Çok süreçli (Multi-process) sekme izolasyonu ve yerleşik izleyici/reklam engelleyici

---

## 🛠️ Derleme ve Test (Quick Start)

Projeyi yerel ortamınızda derlemek ve tüm modüllerin birim testlerini çalıştırmak için **Rust (`cargo`)** kurulu olması yeterlidir:

```bash
# Depoyu klonlayın
git clone https://github.com/EkremTezcanSaridag/anka-browser.git
cd anka-browser

# Tüm çalışma alanını kontrol edin
cargo check

# Tüm modüllerin birim testlerini çalıştırın (anka-url, anka-net, anka-html, anka-css, anka-layout, anka-paint)
cargo test

# Dahili karşılama sayfasıyla motoru çalıştırın
cargo run

# Gerçek bir web sitesine (örn: CERN - Dünyanın ilk web sitesi) bağlanıp 6 istasyondan geçirerek çizin
cargo run -- http://info.cern.ch
```

---

## 🤝 Katkıda Bulunma (Contributing)

**Anka**, açık kaynak topluluğunun gücüyle büyümek üzere tasarlanmıştır! Sıfırdan bir tarayıcı motoru yazmak yüzlerce küçük ve bağımsız parçadan oluşur:
- Yeni bir CSS özelliğinin eklenmesi,
- HTML ayrıştırıcıdaki bir uç durumun (edge-case) çözülmesi,
- Birim testlerinin (unit tests) yazılması veya dokümantasyonun geliştirilmesi.

Projeye katkıda bulunmak için **Issues** sekmesindeki `good first issue` etiketli görevlere göz atabilir, tartışmalara katılabilir ve Pull Request gönderebilirsiniz.

---

## 📄 Lisans

Bu proje açık kaynaklıdır. Lisans detayları repo içerisinde yer almaktadır.
