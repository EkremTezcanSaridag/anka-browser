// Anka Browser - Sayfa Yerleşimi ve Kutu Modeli Motoru (İstasyon 5)

use anka_css::CssRule;
use anka_html::{DomNode, NodeType};

/// Ekrana çizilecek tek bir kutunun koordinatlarını, boyutunu ve stilini tutan paket:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutBox {
    // Hangi HTML etiketi? (örn: "h1", "p", "li")
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

    // Yazının rengi (örn: "red", "black", "#ff5733")
    pub color: String,

    // Kutunun arka plan rengi (varsa, örn: "#f1f5f9")
    pub bg_color: Option<String>,

    // Yazının boyutu (piksel, örn: 32, 16)
    pub font_size: u32,

    // Eğer bu kutu bir link (<a>) ise hedef adresi
    pub href: Option<String>,
}

/// Uzun bir metni pencere genişliğine (`max_width`) ve yazı boyutuna (`font_size`) göre
/// kelimeleri bölmeden otomatik olarak alt satırlara (`Vec<String>`) kaydırır (Word Wrap).
pub fn wrap_text(text: &str, max_width: u32, font_size: u32) -> Vec<String> {
    let char_width = (font_size / 2).max(6);
    let max_chars = (max_width / char_width).max(16) as usize;

    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.is_empty() {
            current_line.push_str(word);
        } else if current_line.chars().count() + 1 + word.chars().count() <= max_chars {
            current_line.push(' ');
            current_line.push_str(word);
        } else {
            lines.push(current_line);
            current_line = word.to_string();
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(text.to_string());
    }

    lines
}

/// HTML ağacını (`DomNode`) ve CSS kurallarını (`CssRule`) birleştirerek
/// her kutunun ekranda duracağı `(x, y, width, height)` koordinatlarını hesaplar.
pub fn compute_layout(root: &DomNode, rules: &[CssRule], viewport_width: u32) -> Vec<LayoutBox> {
    let mut boxes = Vec::new();
    // İlk kutuyu ekranın üstünden 16 piksel aşağıdan başlatıyoruz:
    let mut current_y: u32 = 16;

    layout_node(
        root,
        "body",
        None,
        rules,
        viewport_width,
        &mut current_y,
        &mut boxes,
    );

    boxes
}

/// Ağaçtaki kutuları yukarıdan aşağıya gezerek koordinat dağıtan yardımcı fonksiyon
fn layout_node(
    node: &DomNode,
    current_tag: &str,
    current_href: Option<&str>,
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

            let next_href = node.get_attribute("href").or(current_href);

            for child in &node.children {
                layout_node(
                    child,
                    tag_name,
                    next_href,
                    rules,
                    viewport_width,
                    current_y,
                    boxes,
                );
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

            // B) Varsayılan yazı rengi siyah, arka plan rengi yok (None)
            let mut color = "black".to_string();
            let mut bg_color: Option<String> = None;

            // C) CSS kurallarına bak: Bu etiket (örn: "h1") için özel renk, arka plan veya boyut var mı?
            for rule in rules {
                if rule.selector == current_tag {
                    for decl in &rule.declarations {
                        if decl.property == "color" {
                            color = decl.value.clone();
                        } else if decl.property == "background-color"
                            || decl.property == "background"
                        {
                            bg_color = Some(decl.value.clone());
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

            // D) Liste elemanı (`<li>`) ise başına madde işareti (`• `) koy ve içeriden başlat:
            let is_list_item = current_tag == "li";
            let x = if is_list_item { 32 } else { 16 };
            let width = viewport_width.saturating_sub(x + 16);
            let height = font_size + 8;

            let formatted_text = if is_list_item {
                format!("• {text}")
            } else {
                text.clone()
            };

            // E) Uzun cümleleri pencere genişliğine göre satırlara böl (Word Wrap):
            let lines = wrap_text(&formatted_text, width, font_size);
            let line_count = lines.len();

            for (idx, line) in lines.into_iter().enumerate() {
                let y = *current_y;
                boxes.push(LayoutBox {
                    tag: current_tag.to_string(),
                    text: line,
                    x,
                    y,
                    width,
                    height,
                    color: color.clone(),
                    bg_color: bg_color.clone(),
                    font_size,
                    href: current_href.map(|s| s.to_string()),
                });

                // Aynı paragrafın alt satırları birbirine yakın (4px),
                // yeni bir kutuya geçerken ise 12px boşluk bırak!
                if idx + 1 < line_count {
                    *current_y += height + 4;
                } else {
                    *current_y += height + 12;
                }
            }
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
        let css = "h1 { color: red; background-color: #f1f5f9; font-size: 40px; } p { color: blue; }";

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
        assert_eq!(kutular[0].bg_color, Some("#f1f5f9".to_string()));
        assert_eq!(kutular[0].font_size, 40);
        assert_eq!(kutular[0].height, 48); // 40 + 8

        // 2. Kutu (p): 1. kutunun bittiği yerin 12px altından (16 + 48 + 12 = 76) başlamalı!
        assert_eq!(kutular[1].tag, "p");
        assert_eq!(kutular[1].text, "Yerli Tarayici");
        assert_eq!(kutular[1].y, 76);
        assert_eq!(kutular[1].color, "blue");
        assert_eq!(kutular[1].font_size, 16);
    }

    #[test]
    fn test_uzun_metin_satir_kaydirma_ve_liste() {
        let html = "<ul><li>Bu cok uzun bir liste maddesidir ve dar ekranda alt satira kaymalidir</li></ul>";
        let dom = parse_html(html);
        // 220px dar ekranda satır kaydırmayı test et:
        let kutular = compute_layout(&dom, &[], 220);

        assert!(kutular.len() >= 2);
        assert!(kutular[0].text.starts_with("• "));
        assert_eq!(kutular[0].x, 32);
    }
}
