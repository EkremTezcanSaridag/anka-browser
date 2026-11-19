# 🦅 Anka Browser'a Katkıda Bulunma Rehberi (Contributing Guide)

**Anka Browser** (*Telek Engine*) projesine ilgi gösterdiğiniz için teşekkür ederiz! Bu proje, bir web tarayıcısı motorunun sıfırdan nasıl inşa edildiğini şeffaf, anlaşılır ve modüler bir yapıda geliştirmeyi amaçlayan açık kaynaklı bir topluluk projesidir.

---

## 🏗️ Modüler Mimari (6 İstasyon)

Kod tabanımız `crates/` dizini altında birbirinden bağımsız test edilebilen 6 ana modülden oluşur:

1. **`crates/anka-url`**: WHATWG standartlarına uygun URL ayrıştırıcı.
2. **`crates/anka-net`**: TCP/HTTP ağ katmanı ve yanıt ayrıştırıcı.
3. **`crates/anka-html`**: Hata toleranslı HTML5 Tokenizer ve DOM ağacı oluşturucu.
4. **`crates/anka-css`**: CSS kural ve stil bildirimi ayrıştırıcı.
5. **`crates/anka-layout`**: Kutu modeli (Box Model) ve `(x, y, width, height)` koordinat hesaplayıcı.
6. **`crates/anka-paint` & `crates/anka-browser`**: RGB çizim komutları listesi (Display List) ve ana tarayıcı uygulaması.

---

## ✅ Katkı Kuralları ("Sıfır Hata & Sıfır Uyarı" Prensibi)

Projeye bir **Pull Request (PR)** göndermeden önce bilgisayarınızda şu 3 komutun **hiçbir hata veya sarı uyarı (warning) vermeden** geçtiğinden emin olun:

```bash
# 1. Tüm modüller derleniyor mu?
cargo check --workspace --all-targets

# 2. Tüm birim testleri (ve yeni eklediğiniz özelliğin testi) geçiyor mu?
cargo test --workspace

# 3. Clippy kalite denetiminden sıfır uyarıyla geçiyor mu?
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 🚀 Adım Adım Pull Request (PR) Gönderme Süreci

1. Depoyu kendi hesabınıza **Fork** edin ve bilgisayarınıza klonlayın.
2. Yeni bir özellik veya hata düzeltmesi için yeni bir dal (branch) açın:
   ```bash
   git checkout -b feat/yeni-ozellik-adi
   ```
3. Kodunuzu anlaşılır yorum satırlarıyla yazın ve mutlaka `#[cfg(test)]` bloğuna **en az 1 birim testi (unit test)** ekleyin.
4. Commit mesajınızı **Conventional Commits** formatında yazın:
   - `feat(anka-css): add hex color parsing support`
   - `fix(anka-url): handle trailing slash in host`
   - `docs(readme): update roadmap status`
5. Dalınızı gönderin (`git push`) ve GitHub üzerinden **Pull Request** açın!
