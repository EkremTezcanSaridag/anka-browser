// Anka Browser - CSS Ayrıştırıcı ve Stil Motoru (İstasyon 4 - v0.2.9)
// Gerçek web sitelerindeki /* ... */ yorumlarını, @media / @keyframes / @font-face bloklarını
// ve @import; satırlarını bozulmadan atlayabilen sağlam CSS okuyucusu!

/// Tek bir CSS özelliğini tutan kutu (Örn: "color: red")
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    // Özellik adı (örn: "color", "background", "font-size")
    pub property: String,

    // Özelliğin değeri (örn: "red", "#ffffff", "24px")
    pub value: String,
}

/// Bir CSS kural bloğunu tutan kutu (Örn: "h1 { color: red; font-size: 24px; }")
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssRule {
    // Bu kural hangi etiketi hedefliyor? (örn: "h1", "p", "body")
    pub selector: String,

    // Bu etikete uygulanacak özelliklerin listesi
    pub declarations: Vec<Declaration>,
}

/// CSS metnindeki tüm `/* ... */` yorumlarını temizler.
fn strip_css_comments(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;

    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        if let Some(end) = after_start.find("*/") {
            rest = &after_start[end + 2..];
        } else {
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

/// Tek bir kural bloğunun (`color: red; font-size: 24px;`) özelliklerini ayrıştırır.
fn parse_declarations(declarations_part: &str) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    for item in declarations_part.split(';') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if let Some((prop, val)) = item.split_once(':') {
            let property = prop.trim().to_lowercase();
            let value = val.trim().to_string();
            if !property.is_empty() && !value.is_empty() {
                declarations.push(Declaration { property, value });
            }
        }
    }
    declarations
}

/// Verilen CSS metnini doğrayarak `CssRule` kutularının listesine çevirir.
/// Gerçek web sitelerindeki `/* yorum */` bloklarını, `@media (...) { ... }` iç içe kurallarını
/// ve `@import ...;` satırlarını hatasız yönetir.
pub fn parse_css(raw: &str) -> Vec<CssRule> {
    let cleaned = strip_css_comments(raw);
    let bytes = cleaned.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;
    let mut rules = Vec::new();

    while i < len {
        // Boşlukları atla
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }

        // `@` ile başlayan kurallar (@media, @keyframes, @font-face, @import, @charset)
        if bytes[i] == b'@' {
            // Bir sonraki `{` veya `;` karakterine kadar ilerle
            while i < len && bytes[i] != b'{' && bytes[i] != b';' {
                i += 1;
            }
            if i < len && bytes[i] == b';' {
                i += 1; // `@import ...;` gibi noktalı virgülle biten satırı geç
                continue;
            }
            if i < len && bytes[i] == b'{' {
                // Parantez derinliğini sayarak tüm iç içe `@media { ... { ... } }` bloğunu atla!
                let mut depth = 1usize;
                i += 1;
                while i < len && depth > 0 {
                    if bytes[i] == b'{' {
                        depth += 1;
                    } else if bytes[i] == b'}' {
                        depth -= 1;
                    }
                    i += 1;
                }
            }
            continue;
        }

        // Normal CSS kuralı: `selector { declarations }`
        let sel_start = i;
        while i < len && bytes[i] != b'{' && bytes[i] != b'}' {
            i += 1;
        }
        if i >= len {
            break;
        }
        if bytes[i] == b'}' {
            i += 1;
            continue;
        }

        let selector = cleaned[sel_start..i].trim().to_lowercase();
        i += 1; // '{' geç

        let decl_start = i;
        let mut depth = 1usize;
        while i < len && depth > 0 {
            if bytes[i] == b'{' {
                depth += 1;
            } else if bytes[i] == b'}' {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            i += 1;
        }
        let decl_end = i;
        if i < len && bytes[i] == b'}' {
            i += 1;
        }

        if !selector.is_empty() {
            let declarations = parse_declarations(&cleaned[decl_start..decl_end]);
            rules.push(CssRule {
                selector,
                declarations,
            });
        }
    }

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tek_css_kurali_dograma() {
        let css = "h1 { color: red; font-size: 24px; }";
        let kurallar = parse_css(css);

        assert_eq!(kurallar.len(), 1);
        assert_eq!(kurallar[0].selector, "h1");
        assert_eq!(kurallar[0].declarations.len(), 2);

        assert_eq!(
            kurallar[0].declarations[0],
            Declaration {
                property: "color".to_string(),
                value: "red".to_string(),
            }
        );
        assert_eq!(
            kurallar[0].declarations[1],
            Declaration {
                property: "font-size".to_string(),
                value: "24px".to_string(),
            }
        );
    }

    #[test]
    fn test_birden_fazla_css_kurali() {
        let css = "
            body { background: white; }
            p { color: black; }
        ";
        let kurallar = parse_css(css);

        assert_eq!(kurallar.len(), 2);
        assert_eq!(kurallar[0].selector, "body");
        assert_eq!(kurallar[1].selector, "p");
    }

    #[test]
    fn test_yorumlar_ve_media_bloklari_bozmamali() {
        let css = "
            /* Ana sayfa stilleri */
            @import url('foo.css');
            @media (max-width: 600px) {
                body { background: black; }
                p { font-size: 10px; }
            }
            h1 { color: rgb(15, 23, 42); }
        ";
        let kurallar = parse_css(css);

        assert_eq!(kurallar.len(), 1);
        assert_eq!(kurallar[0].selector, "h1");
        assert_eq!(kurallar[0].declarations[0].value, "rgb(15, 23, 42)");
    }
}
