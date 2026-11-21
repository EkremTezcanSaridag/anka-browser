// Anka Browser - Sayfa Yerleşimi ve Kutu Modeli Motoru (İstasyon 5)

use anka_css::{CssRule, Declaration, parse_css};
use anka_html::{DomNode, NodeType};

/// Ekrana çizilecek tek bir kutunun koordinatlarını, boyutunu ve stilini tutan paket:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutBox {
    // Hangi HTML etiketi? (örn: "h1", "p", "li", "hr")
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

    // Kutunun çerçeve (border) rengi (varsa, örn: "#cbd5e1")
    pub border_color: Option<String>,

    // Yazının boyutu (piksel, örn: 32, 16)
    pub font_size: u32,

    // Eğer bu kutu bir link (<a>) ise hedef adresi
    pub href: Option<String>,
}

/// Ağaçta yukarıdan aşağıya inerken ebeveyn etiketlerden miras alınan bağlam bilgisi:
#[derive(Clone, Default)]
struct NodeContext {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
    inline_declarations: Vec<Declaration>,
    href: Option<String>,
}

/// Bir CSS seçicisinin (`h1`, `.kart`, `#baslik`, `div.kart`, `h1, h2`)
/// o anki HTML düğümüyle eşleşip eşleşmediğini kontrol eder.
pub fn selector_matches(selector: &str, tag: &str, id: Option<&str>, classes: &[String]) -> bool {
    for part in selector.split(',') {
        let s = part.trim();
        if s.is_empty() {
            continue;
        }
        // 1. ".kart" -> class eşleşmesi
        if let Some(cls) = s.strip_prefix('.') {
            if classes.iter().any(|c| c == cls) {
                return true;
            }
        }
        // 2. "#baslik" -> id eşleşmesi
        else if let Some(target_id) = s.strip_prefix('#') {
            if id == Some(target_id) {
                return true;
            }
        }
        // 3. "div.kart" -> hem etiket hem class eşleşmesi
        else if let Some((t, cls)) = s.split_once('.') {
            if t == tag && classes.iter().any(|c| c == cls) {
                return true;
            }
        }
        // 4. "h1" -> doğrudan etiket eşleşmesi
        else if s == tag {
            return true;
        }
    }
    false
}

/// CSS `border: 1px solid #cbd5e1` veya `border-color: red` değerinden rengi ayıklar.
fn extract_border_color(val: &str) -> Option<String> {
    let last_token = val.split_whitespace().last()?.trim();
    if last_token.is_empty() || last_token == "none" || last_token == "0" {
        None
    } else {
        Some(last_token.to_string())
    }
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
    let mut current_y: u32 = 16;
    let mut inline_x: u32 = 16;
    let mut last_inline_height: u32 = 0;

    let initial_ctx = NodeContext {
        tag: "body".to_string(),
        ..Default::default()
    };

    layout_node(
        root,
        &initial_ctx,
        rules,
        viewport_width,
        &mut current_y,
        &mut inline_x,
        &mut last_inline_height,
        &mut boxes,
    );

    boxes
}

#[allow(clippy::too_many_arguments)]
fn layout_node(
    node: &DomNode,
    ctx: &NodeContext,
    rules: &[CssRule],
    viewport_width: u32,
    current_y: &mut u32,
    inline_x: &mut u32,
    last_inline_height: &mut u32,
    boxes: &mut Vec<LayoutBox>,
) {
    match &node.node_type {
        NodeType::Element(tag_name) => {
            if matches!(tag_name.as_str(), "head" | "style" | "script" | "title") {
                return;
            }

            // `<hr>` etiketi: Boydan boya yatay ayırıcı çizgi kutusu!
            if tag_name == "hr" {
                if *inline_x > 16 {
                    *current_y += *last_inline_height + 12;
                    *inline_x = 16;
                    *last_inline_height = 0;
                }
                boxes.push(LayoutBox {
                    tag: "hr".to_string(),
                    text: String::new(),
                    x: 16,
                    y: *current_y + 4,
                    width: viewport_width.saturating_sub(32),
                    height: 2,
                    color: "#cbd5e1".to_string(),
                    bg_color: Some("#cbd5e1".to_string()),
                    border_color: None,
                    font_size: 4,
                    href: None,
                });
                *current_y += 18;
                return;
            }

            // Blok etiketine girerken açıkta kalan yan yana (inline) satırı kapat:
            let is_inline_tag = matches!(tag_name.as_str(), "a" | "span" | "button" | "b" | "i");
            if !is_inline_tag && *inline_x > 16 {
                *current_y += *last_inline_height + 12;
                *inline_x = 16;
                *last_inline_height = 0;
            }

            // Etiketin class="...", id="...", style="...", href="..." özelliklerini miras al:
            let mut next_ctx = ctx.clone();
            next_ctx.tag = tag_name.clone();

            if let Some(id_val) = node.get_attribute("id") {
                next_ctx.id = Some(id_val.trim().to_lowercase());
            }
            if let Some(class_val) = node.get_attribute("class") {
                for c in class_val.split_whitespace() {
                    let c_low = c.to_lowercase();
                    if !next_ctx.classes.contains(&c_low) {
                        next_ctx.classes.push(c_low);
                    }
                }
            }
            if let Some(style_val) = node.get_attribute("style") {
                let parsed = parse_css(&format!("inline {{ {style_val} }}"));
                if let Some(first_rule) = parsed.into_iter().next() {
                    next_ctx.inline_declarations.extend(first_rule.declarations);
                }
            }
            if let Some(href_val) = node.get_attribute("href") {
                next_ctx.href = Some(href_val.to_string());
            }

            for child in &node.children {
                layout_node(
                    child,
                    &next_ctx,
                    rules,
                    viewport_width,
                    current_y,
                    inline_x,
                    last_inline_height,
                    boxes,
                );
            }

            if !is_inline_tag && *inline_x > 16 {
                *current_y += *last_inline_height + 12;
                *inline_x = 16;
                *last_inline_height = 0;
            }
        }

        NodeType::Text(text) => {
            let current_tag = ctx.tag.as_str();
            let mut font_size: u32 = match current_tag {
                "h1" => 32,
                "h2" => 24,
                "h3" => 20,
                _ => 16,
            };
            let mut color = "black".to_string();
            let mut bg_color: Option<String> = None;
            let mut border_color: Option<String> = None;

            let apply_decl = |decl: &Declaration,
                              color: &mut String,
                              bg_color: &mut Option<String>,
                              border_color: &mut Option<String>,
                              font_size: &mut u32| {
                match decl.property.as_str() {
                    "color" => *color = decl.value.clone(),
                    "background-color" | "background" => *bg_color = Some(decl.value.clone()),
                    "border" | "border-color" => {
                        *border_color = extract_border_color(&decl.value);
                    }
                    "font-size" => {
                        let sayi_str = decl.value.trim_end_matches("px").trim();
                        if let Ok(boyut) = sayi_str.parse::<u32>() {
                            *font_size = boyut;
                        }
                    }
                    _ => {}
                }
            };

            // 1. Sayfa CSS kurallarını (etiket, .class, #id) sırayla uygula:
            for rule in rules {
                if selector_matches(
                    &rule.selector,
                    current_tag,
                    ctx.id.as_deref(),
                    &ctx.classes,
                ) {
                    for decl in &rule.declarations {
                        apply_decl(
                            decl,
                            &mut color,
                            &mut bg_color,
                            &mut border_color,
                            &mut font_size,
                        );
                    }
                }
            }

            // 2. Etiketin kendi üzerindeki `style="..."` özelliğini en yüksek öncelikle uygula:
            for decl in &ctx.inline_declarations {
                apply_decl(
                    decl,
                    &mut color,
                    &mut bg_color,
                    &mut border_color,
                    &mut font_size,
                );
            }

            let height = font_size + 10;
            let char_w = (font_size / 2).max(7);

            // YAN YANA (INLINE) DİZİLİM:
            // Kısa `<a>`, `<span>`, `<button>` öğeleri satıra sığıyorsa yan yana diz!
            let is_short_inline = matches!(current_tag, "a" | "span" | "button")
                && text.chars().count() <= 28;

            if is_short_inline {
                let item_w = (text.chars().count() as u32 * char_w) + 20;
                if *inline_x + item_w > viewport_width.saturating_sub(16) && *inline_x > 16 {
                    // Satır doldu, bir alt satırın başına geç!
                    *current_y += (*last_inline_height).max(height) + 10;
                    *inline_x = 16;
                }

                boxes.push(LayoutBox {
                    tag: current_tag.to_string(),
                    text: text.clone(),
                    x: *inline_x,
                    y: *current_y,
                    width: item_w,
                    height,
                    color,
                    bg_color,
                    border_color,
                    font_size,
                    href: ctx.href.clone(),
                });

                *inline_x += item_w + 12;
                *last_inline_height = (*last_inline_height).max(height);
                return;
            }

            // Normal blok kutuya geçmeden önce açık inline satırı varsa kapat:
            if *inline_x > 16 {
                *current_y += *last_inline_height + 12;
                *inline_x = 16;
                *last_inline_height = 0;
            }

            let is_list_item = current_tag == "li";
            let x = if is_list_item { 32 } else { 16 };
            let width = viewport_width.saturating_sub(x + 16);

            let formatted_text = if is_list_item {
                format!("• {text}")
            } else {
                text.clone()
            };

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
                    border_color: border_color.clone(),
                    font_size,
                    href: ctx.href.clone(),
                });

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

        let kutular = compute_layout(&dom, &kurallar, 800);

        assert_eq!(kutular.len(), 2);
        assert_eq!(kutular[0].tag, "h1");
        assert_eq!(kutular[0].text, "Merhaba Anka!");
        assert_eq!(kutular[0].x, 16);
        assert_eq!(kutular[0].y, 16);
        assert_eq!(kutular[0].color, "red");
        assert_eq!(kutular[0].bg_color, Some("#f1f5f9".to_string()));
        assert_eq!(kutular[0].font_size, 40);
        assert_eq!(kutular[0].height, 50); // 40 + 10

        assert_eq!(kutular[1].tag, "p");
        assert_eq!(kutular[1].text, "Yerli Tarayici");
        assert_eq!(kutular[1].y, 78); // 16 + 50 + 12
        assert_eq!(kutular[1].color, "blue");
    }

    #[test]
    fn test_uzun_metin_satir_kaydirma_ve_liste() {
        let html = "<ul><li>Bu cok uzun bir liste maddesidir ve dar ekranda alt satira kaymalidir</li></ul>";
        let dom = parse_html(html);
        let kutular = compute_layout(&dom, &[], 220);

        assert!(kutular.len() >= 2);
        assert!(kutular[0].text.starts_with("• "));
        assert_eq!(kutular[0].x, 32);
    }

    #[test]
    fn test_class_id_border_ve_yanyana_linkler() {
        let html = "<body><nav><a href=\"/1\">Menu 1</a><a href=\"/2\">Menu 2</a></nav><div class=\"kart\" id=\"ozel\"><p>Kart Icerigi</p></div></body>";
        let css = ".kart { background-color: #e0f2fe; border: 1px solid #0284c7; } #ozel { color: #0369a1; }";

        let dom = parse_html(html);
        let kurallar = parse_css(css);
        let kutular = compute_layout(&dom, &kurallar, 800);

        assert_eq!(kutular.len(), 3);
        // İlk iki menü linki aynı satırda (y = 16) yan yana (farklı x) durmalı!
        assert_eq!(kutular[0].y, 16);
        assert_eq!(kutular[1].y, 16);
        assert!(kutular[1].x > kutular[0].x);

        // 3. kutu (.kart ve #ozel stillerini miras almış olmalı!)
        assert_eq!(kutular[2].bg_color, Some("#e0f2fe".to_string()));
        assert_eq!(kutular[2].border_color, Some("#0284c7".to_string()));
        assert_eq!(kutular[2].color, "#0369a1");
    }
}
