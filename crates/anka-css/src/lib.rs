// Anka Browser - CSS Ayrıştırıcı ve Stil Motoru (İstasyon 4)

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

/// Verilen düz CSS metnini doğrayarak `CssRule` kutularının listesine çevirir.
/// Örnek girdi: "h1 { color: red; font-size: 24px; } p { color: black; }"
pub fn parse_css(raw: &str) -> Vec<CssRule> {
    let mut rules = Vec::new();

    // 1. ADIM: Her kural '}' işaretiyle biter. Metni '}' işaretlerinden bloklara böl:
    for block in raw.split('}') {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }

        // 2. ADIM: Bloğu '{' işaretinden ikiye kes:
        // Sol taraf = Seçici (selector, örn: "h1")
        // Sağ taraf = Özellikler (örn: "color: red; font-size: 24px;")
        let Some((selector_part, declarations_part)) = block.split_once('{') else {
            continue;
        };

        let selector = selector_part.trim().to_lowercase();
        if selector.is_empty() {
            continue;
        }

        // 3. ADIM: Parantez içindeki özellikleri ';' işaretinden satırlara böl:
        let mut declarations = Vec::new();
        for item in declarations_part.split(';') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }

            // 4. ADIM: Her satırı ':' işaretinden ikiye kes:
            // Sol taraf = property ("color"), Sağ taraf = value ("red")
            if let Some((prop, val)) = item.split_once(':') {
                let property = prop.trim().to_lowercase();
                let value = val.trim().to_string();

                if !property.is_empty() && !value.is_empty() {
                    declarations.push(Declaration { property, value });
                }
            }
        }

        rules.push(CssRule {
            selector,
            declarations,
        });
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
}
