// Anka Browser - Sayfa Yerleşimi ve Kutu Modeli Motoru (İstasyon 5)

use anka_css::CssRule;
use anka_html::{DomNode, NodeType};

/// Ekrana çizilecek tek bir kutunun koordinatlarını, boyutunu ve stilini tutan paket:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutBox {
    // Hangi HTML etiketi? (örn: "h1", "p")
    pub tag: String,

    // Kutunun içindeki yazı (örn: "Merhaba Anka!")
    pub text: String,

    // Ekranın sol kenarından uzaklığı (piksel)
    pub x: u32,

    // Ekranın üst kenarından uzaklığı (piksel)
    pub y: u32,

    // Kutunun genişliği (piksel)
    pub width: u32,

    // Kutunun yüksekliği (piksel)
    pub height: u32,

    // Yazının rengi (örn: "red", "black", "blue")
    pub color: String,

    // Yazının boyutu (piksel, örn: 32, 16)
    pub font_size: u32,
}

/// HTML ağacını (`DomNode`) ve CSS kurallarını (`CssRule`) birleştirerek
/// her kutunun ekranda duracağı `(x, y, width, height)` koordinatlarını hesaplar.
pub fn compute_layout(root: &DomNode, rules: &[CssRule], viewport_width: u32) -> Vec<LayoutBox> {
    let mut boxes = Vec::new();
    // İlk kutuyu ekranın üstünden 16 piksel aşağıdan başlatıyoruz:
    let mut current_y: u32 = 16;

    layout_node(root, "body", rules, viewport_width, &mut current_y, &mut boxes);

    boxes
}

/// Ağaçtaki kutuları yukarıdan aşağıya gezerek koordinat dağıtan yardımcı fonksiyon
fn layout_node(
    node: &DomNode,
    current_tag: &str,
    rules: &[CssRule],
    viewport_width: u32,
    current_y: &mut u32,
    boxes: &mut Vec<LayoutBox>,
) {
    match &node.node_type {
        // 1. DURUM: Bu bir Etiket kutusuysa (örn: <h1> veya <p>),
        // içindeki çocukları gezerken etiket adını ("h1") onlara bildir:
        NodeType::Element(tag_name) => {
            // <head>, <style>, <script>, <title> içindeki metinler sayfa gövdesine çizilmez
            if matches!(tag_name.as_str(), "head" | "style" | "script" | "title") {
                return;
            }

            for child in &node.children {
                layout_node(child, tag_name, rules, viewport_width, current_y, boxes);
            }
        }

        // 2. DURUM: Bu bir Yazı (Text) düğümüyse, ekranda koordinat verip kutu oluştur!
        NodeType::Text(text) => {
            // A) Varsayılan yazı boyutunu etikete göre belirle (h1 büyük, p normal)
            let mut font_size: u32 = match current_tag {
                "h1" => 32,
                "h2" => 24,
                "h3" => 20,
                _ => 16,
            };

            // B) Varsayılan yazı rengi siyah olsun
            let mut color = "black".to_string();

            // C) CSS kurallarına bak: Bu etiket (örn: "h1") için özel renk veya boyut var mı?
            for rule in rules {
                if rule.selector == current_tag {
                    for decl in &rule.declarations {
                        if decl.property == "color" {
                            color = decl.value.clone();
                        } else if decl.property == "font-size" {
                            // "24px" yazısının sonundaki "px" harflerini silip sayıya çevir
                            let sayi_str = decl.value.trim_end_matches("px").trim();
                            if let Ok(boyut) = sayi_str.parse::<u32>() {
                                font_size = boyut;
                            }
                        }
                    }
                }
            }

            // D) Kutunun ekran koordinatlarını hesapla:
            let x = 16; // Soldan 16 piksel boşluk
            let y = *current_y;
            let width = viewport_width.saturating_sub(32); // Sağdan ve soldan 16'şar piksel boşluk
            let height = font_size + 8; // Yazı boyutuna 8 piksel satır payı ekle

            boxes.push(LayoutBox {
                tag: current_tag.to_string(),
                text: text.clone(),
                x,
                y,
                width,
                height,
                color,
                font_size,
            });

            // E) Bir sonraki kutu bunun üstüne binmesin diye `current_y` metresini aşağı kaydır!
            *current_y += height + 12;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anka_css::parse_css;
    use anka_html::parse_html;

    #[test]
    fn test_html_ve_css_birlesik_yerlesim() {
        let html = "<body><h1>Merhaba Anka!</h1><p>Yerli Tarayici</p></body>";
        let css = "h1 { color: red; font-size: 40px; } p { color: blue; }";

        let dom = parse_html(html);
        let kurallar = parse_css(css);

        // 800 piksel genişliğindeki bir ekran için yerleşimi hesapla:
        let kutular = compute_layout(&dom, &kurallar, 800);

        assert_eq!(kutular.len(), 2);

        // 1. Kutu (h1): y = 16'dan başlamalı, rengi CSS'ten "red", boyutu 40 olmalı
        assert_eq!(kutular[0].tag, "h1");
        assert_eq!(kutular[0].text, "Merhaba Anka!");
        assert_eq!(kutular[0].x, 16);
        assert_eq!(kutular[0].y, 16);
        assert_eq!(kutular[0].color, "red");
        assert_eq!(kutular[0].font_size, 40);
        assert_eq!(kutular[0].height, 48); // 40 + 8

        // 2. Kutu (p): 1. kutunun bittiği yerin 12px altından (16 + 48 + 12 = 76) başlamalı!
        assert_eq!(kutular[1].tag, "p");
        assert_eq!(kutular[1].text, "Yerli Tarayici");
        assert_eq!(kutular[1].y, 76);
        assert_eq!(kutular[1].color, "blue");
        assert_eq!(kutular[1].font_size, 16); // p için varsayılan boyut
    }
}
