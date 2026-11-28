// Anka Browser - Sayfa Yerleşimi, CSS Kutu Modeli ve Flexbox Motoru (İstasyon 5 - v0.2.6)
// Desteklenen özellikler:
// - Seçici eşleştirme (`tag`, `.class`, `#id`, `tag.class`, `h1, h2`) ve satır içi `style="..."`
// - CSS Kutu Modeli: `padding`, `margin`, `text-align` (`left` | `center` | `right`), `background-color`, `border`
// - Çok satırlı kart / paragraf birleştirme (satırlar ayrı çerçevelere bölünmez, tek kart içinde toplanır!)
// - CSS Flexbox (`display: flex` ve `gap`): Çocuk kutuları (kartlar, sütunlar) yatayda eşit genişlikle yan yana dizer!
// - Otomatik satır kaydırma (`wrap_text`), `<li>`, `<hr>`, `<img src alt width height>` ve inline `<a>`/`<span>`

use std::collections::HashMap;

use anka_css::{CssRule, Declaration, parse_css};
use anka_html::{DomNode, NodeType};

/// Ekrana çizilecek tek bir kutunun koordinatlarını, boyutunu ve stilini tutan paket:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutBox {
    // Hangi HTML etiketi? (örn: "h1", "p", "li", "hr", "img", "box")
    pub tag: String,

    // Kutunun içindeki yazı (örn: "Merhaba Anka!") — arka plan kapsayıcı kutularda boş olabilir
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

    // CSS `padding` (kutu iç boşluğu, piksel)
    pub padding: u32,

    // CSS `text-align` ("left", "center", "right")
    pub text_align: String,

    // Eğer bu kutu bir link (<a>) ise hedef adresi
    pub href: Option<String>,

    // Eğer bu kutu bir görsel (<img>) ise kaynak adresi (`src="..."`)
    pub image_src: Option<String>,

    // Eğer bu kutuda JavaScript tıklama olayı (`onclick="..."`) varsa çalıştırılacak JS kodu
    pub onclick: Option<String>,

    // HTML `<form action="..." method="GET|POST">` içindeki metin/şifre kutusu (`<input>`) ve gönderme bilgileri:
    pub input_name: Option<String>,
    pub form_action: Option<String>,
    pub is_submit: bool,
    pub is_password: bool,
    pub is_post_form: bool,
    pub is_multiline: bool,
    pub border_radius: u32,
    pub has_shadow: bool,
    pub is_bold: bool,
    pub is_italic: bool,
}

/// CSS genişlik / maksimum genişlik ölçüsü (`px` veya `%`):
#[derive(Clone, Copy)]
enum SizeSpec {
    Px(u32),
    Percent(u32),
}

impl SizeSpec {
    fn parse(val: &str) -> Option<Self> {
        let temiz = val.split_whitespace().next()?.trim().to_lowercase();
        if let Some(p) = temiz.strip_suffix('%') {
            return p.trim().parse::<u32>().ok().map(SizeSpec::Percent);
        }
        if let Some(px) = temiz.strip_suffix("px") {
            return px.trim().parse::<u32>().ok().map(SizeSpec::Px);
        }
        temiz.parse::<u32>().ok().map(SizeSpec::Px)
    }

    fn resolve(self, parent_width: u32) -> u32 {
        match self {
            SizeSpec::Px(px) => px,
            SizeSpec::Percent(pct) => (parent_width * pct.min(100)) / 100,
        }
    }
}

/// Bir HTML düğümüne uygulanmış CSS özelliklerinin çözümlenmiş paketi:
#[derive(Clone)]
struct ResolvedStyle {
    color: String,
    bg_color: Option<String>,
    border_color: Option<String>,
    border_radius: u32,
    has_shadow: bool,
    font_bold: Option<bool>,
    font_italic: Option<bool>,
    font_size: u32,
    padding: u32,
    margin: u32,
    gap: u32,
    width: Option<SizeSpec>,
    max_width: Option<SizeSpec>,
    display: String,
    text_align: String,
}

impl Default for ResolvedStyle {
    fn default() -> Self {
        Self {
            color: "black".to_string(),
            bg_color: None,
            border_color: None,
            border_radius: 0,
            has_shadow: false,
            font_bold: None,
            font_italic: None,
            font_size: 16,
            padding: 0,
            margin: 0,
            gap: 12,
            width: None,
            max_width: None,
            display: "block".to_string(),
            text_align: "left".to_string(),
        }
    }
}

/// Birleştirici (combinator: `.menu a`, `div > p`) eşleştirme için ata düğüm kimliği:
#[derive(Clone)]
struct AncestorInfo {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
}

/// Ağaçta yukarıdan aşağıya inerken ebeveyn etiketlerden miras alınan bağlam bilgisi:
#[derive(Clone, Default)]
struct NodeContext {
    tag: String,
    href: Option<String>,
    onclick: Option<String>,
    form_action: Option<String>,
    form_method_post: bool,
    text_align: Option<String>,
    inherited_color: Option<String>,
    inherited_bold: Option<bool>,
    inherited_italic: Option<bool>,
    current_style: ResolvedStyle,
    ancestors: Vec<AncestorInfo>,
    /// Üst kapsayıcı (örn: `.kart` veya `#duyuru`) zaten arka plan/çerçeve çizdiyse çocuk metinler tekrar çerçeve çizmez!
    container_painted_bg: bool,
}

fn single_selector_matches(token: &str, tag: &str, id: Option<&str>, classes: &[String]) -> bool {
    let s = if let Some((base, pseudo)) = token.split_once(':') {
        if matches!(pseudo, "link" | "visited") {
            base
        } else {
            return false;
        }
    } else {
        token
    };
    if s.is_empty() || s.contains('[') {
        return false;
    }
    if s == "*" {
        return true;
    }
    if let Some(cls) = s.strip_prefix('.') {
        classes.iter().any(|c| c == cls)
    } else if let Some(target_id) = s.strip_prefix('#') {
        id == Some(target_id)
    } else if let Some((t, cls)) = s.split_once('.') {
        t == tag && classes.iter().any(|c| c == cls)
    } else if let Some((t, target_id)) = s.split_once('#') {
        t == tag && id == Some(target_id)
    } else {
        s == tag
    }
}

fn selector_part_matches(
    part: &str,
    tag: &str,
    id: Option<&str>,
    classes: &[String],
    ancestors: &[AncestorInfo],
) -> bool {
    let normalized = part.replace('>', " > ");
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let Some(&last) = tokens.last() else {
        return false;
    };
    if last == ">" || !single_selector_matches(last, tag, id, classes) {
        return false;
    }

    let mut t_idx = tokens.len() - 1;
    let mut a_idx = ancestors.len();

    while t_idx > 0 {
        t_idx -= 1;
        if tokens[t_idx] == ">" {
            if t_idx == 0 || a_idx == 0 {
                return false;
            }
            t_idx -= 1;
            a_idx -= 1;
            let parent = &ancestors[a_idx];
            if !single_selector_matches(
                tokens[t_idx],
                &parent.tag,
                parent.id.as_deref(),
                &parent.classes,
            ) {
                return false;
            }
        } else {
            let mut matched = false;
            while a_idx > 0 {
                a_idx -= 1;
                let anc = &ancestors[a_idx];
                if single_selector_matches(
                    tokens[t_idx],
                    &anc.tag,
                    anc.id.as_deref(),
                    &anc.classes,
                ) {
                    matched = true;
                    break;
                }
            }
            if !matched {
                return false;
            }
        }
    }

    true
}

fn selector_matches_with_ancestors(
    selector: &str,
    tag: &str,
    id: Option<&str>,
    classes: &[String],
    ancestors: &[AncestorInfo],
) -> bool {
    selector
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .any(|part| selector_part_matches(part, tag, id, classes, ancestors))
}

/// Bir CSS seçicisinin (`h1`, `.kart`, `#baslik`, `div.kart`, `h1, h2`)
/// o anki HTML düğümüyle eşleşip eşleşmediğini kontrol eder.
pub fn selector_matches(selector: &str, tag: &str, id: Option<&str>, classes: &[String]) -> bool {
    selector_matches_with_ancestors(selector, tag, id, classes, &[])
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

/// CSS piksel değerini (`"12px"`, `"16"`, `"10px 20px"` -> ilk sayı `10`) güvenli şekilde `u32` olarak çözer.
fn parse_px(val: &str) -> Option<u32> {
    let first = val.split_whitespace().next()?.trim();
    first.trim_end_matches("px").trim().parse::<u32>().ok()
}

/// Tek bir CSS bildirimini (`Declaration`) `ResolvedStyle` yapısına uygular.
fn apply_declaration(decl: &Declaration, style: &mut ResolvedStyle) {
    match decl.property.as_str() {
        "color" => {
            let trimmed = decl.value.trim();
            let lower = trimmed.to_lowercase();
            let clean = lower.strip_suffix("!important").unwrap_or(&lower).trim();
            if !matches!(clean, "inherit" | "initial" | "unset" | "currentcolor") {
                style.color = trimmed.to_string();
            }
        }
        "background-color" | "background" => {
            let trimmed = decl.value.trim();
            let lower = trimmed.to_lowercase();
            let clean = lower.strip_suffix("!important").unwrap_or(&lower).trim();
            if matches!(clean, "transparent" | "none" | "initial" | "unset") {
                style.bg_color = None;
            } else {
                style.bg_color = Some(trimmed.to_string());
            }
        }
        "border" | "border-color" => style.border_color = extract_border_color(&decl.value),
        "border-radius" => {
            if let Some(px) = parse_px(&decl.value) {
                style.border_radius = px.min(32);
            }
        }
        "font-size" => {
            if let Some(px) = parse_px(&decl.value) {
                style.font_size = px.clamp(10, 64);
            }
        }
        "padding" | "padding-top" | "padding-left" => {
            if let Some(px) = parse_px(&decl.value) {
                style.padding = px.min(48);
            }
        }
        "margin" | "margin-top" | "margin-bottom" => {
            if let Some(px) = parse_px(&decl.value) {
                style.margin = px.min(48);
            }
        }
        "gap" => {
            if let Some(px) = parse_px(&decl.value) {
                style.gap = px.min(48);
            }
        }
        "width" => style.width = SizeSpec::parse(&decl.value),
        "max-width" => style.max_width = SizeSpec::parse(&decl.value),
        "display" => style.display = decl.value.trim().to_lowercase(),
        "text-align" => style.text_align = decl.value.trim().to_lowercase(),
        "box-shadow" => {
            let v = decl.value.trim().to_lowercase();
            style.has_shadow = !v.is_empty() && !matches!(v.as_str(), "none" | "initial" | "unset");
        }
        "font-weight" => {
            let v = decl.value.trim().to_lowercase();
            let clean = v.strip_suffix("!important").unwrap_or(&v).trim();
            if matches!(clean, "bold" | "bolder" | "600" | "700" | "800" | "900") {
                style.font_bold = Some(true);
            } else if matches!(clean, "normal" | "lighter" | "300" | "400") {
                style.font_bold = Some(false);
            }
        }
        "font-style" => {
            let v = decl.value.trim().to_lowercase();
            let clean = v.strip_suffix("!important").unwrap_or(&v).trim();
            if matches!(clean, "italic" | "oblique") {
                style.font_italic = Some(true);
            } else if clean == "normal" {
                style.font_italic = Some(false);
            }
        }
        _ => {}
    }
}

fn effective_box_width(avail_width: u32, style: &ResolvedStyle) -> u32 {
    let mut w = avail_width;
    if let Some(spec) = style.width {
        w = spec.resolve(avail_width).clamp(80, avail_width.max(80));
    }
    if let Some(max_spec) = style.max_width {
        w = w.min(max_spec.resolve(avail_width).max(80));
    }
    w
}

/// Belirli bir elementin (etiket + id + class + atalar + inline style) kendi doğrudan CSS stillerini hesaplar.
fn resolve_element_style(
    tag: &str,
    id: Option<&str>,
    classes: &[String],
    ancestors: &[AncestorInfo],
    inline_decls: &[Declaration],
    rules: &[CssRule],
) -> ResolvedStyle {
    let mut style = ResolvedStyle {
        font_size: match tag {
            "h1" => 32,
            "h2" => 24,
            "h3" => 20,
            "small" => 13,
            _ => 16,
        },
        ..Default::default()
    };

    for rule in rules {
        if selector_matches_with_ancestors(&rule.selector, tag, id, classes, ancestors) {
            for decl in &rule.declarations {
                apply_declaration(decl, &mut style);
            }
        }
    }
    for decl in inline_decls {
        apply_declaration(decl, &mut style);
    }

    style
}

/// Bir element düğümünün doğrudan `id`, `class` ve satır içi `style` özelliklerini ayıklar.
fn extract_own_attrs(node: &DomNode) -> (Option<String>, Vec<String>, Vec<Declaration>) {
    let id = node
        .get_attribute("id")
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty());
    let classes: Vec<String> = node
        .get_attribute("class")
        .map(|v| v.split_whitespace().map(str::to_lowercase).collect())
        .unwrap_or_default();
    let mut inline_decls = Vec::new();
    if let Some(style_val) = node.get_attribute("style") {
        let parsed = parse_css(&format!("inline {{ {style_val} }}"));
        if let Some(first_rule) = parsed.into_iter().next() {
            inline_decls.extend(first_rule.declarations);
        }
    }
    (id, classes, inline_decls)
}

/// Uzun bir metni kutu genişliğine (`max_width`) ve yazı boyutuna (`font_size`) göre
/// kelimeleri bölmeden otomatik olarak alt satırlara (`Vec<String>`) kaydırır (Word Wrap).
pub fn wrap_text(text: &str, max_width: u32, font_size: u32) -> Vec<String> {
    let char_width = ((font_size * 58) / 100).max(7);
    let max_chars = (max_width / char_width).max(12) as usize;

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

/// Bir HTML sayfasının `<body>` etiketine uygulanan arka plan rengini (`background-color` / `background`)
/// CSS kurallarından, satır içi `style="..."` veya `bgcolor="..."` özelliğinden çözer.
pub fn resolve_body_background(root: &DomNode, rules: &[CssRule]) -> Option<String> {
    let html_ancestors = vec![AncestorInfo {
        tag: "html".to_string(),
        id: None,
        classes: Vec::new(),
    }];
    if let Some(body) = find_body_node(root) {
        let (id, classes, inline) = extract_own_attrs(body);
        let style = resolve_element_style(
            "body",
            id.as_deref(),
            &classes,
            &html_ancestors,
            &inline,
            rules,
        );
        if style.bg_color.is_some() {
            return style.bg_color;
        }
        if let Some(bgcolor) = body.get_attribute("bgcolor") {
            let trimmed = bgcolor.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    let style = resolve_element_style("body", None, &[], &html_ancestors, &[], rules);
    style.bg_color
}

fn find_body_node(node: &DomNode) -> Option<&DomNode> {
    if let NodeType::Element(tag) = &node.node_type
        && tag == "body"
    {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_body_node(child) {
            return Some(found);
        }
    }
    None
}

/// HTML ağacını (`DomNode`) ve CSS kurallarını (`CssRule`) birleştirerek
/// her kutunun ekranda duracağı `(x, y, width, height)` koordinatlarını hesaplar.
pub fn compute_layout(root: &DomNode, rules: &[CssRule], viewport_width: u32) -> Vec<LayoutBox> {
    compute_layout_with_images(root, rules, viewport_width, &HashMap::new())
}

/// İndirilmiş görsellerin gerçek piksel boyutlarını (`image_sizes`) da dikkate alarak
/// yerleşim hesaplar; böylece `width`/`height` yazılmamış `<img>` etiketleri doğal oranını korur!
pub fn compute_layout_with_images(
    root: &DomNode,
    rules: &[CssRule],
    viewport_width: u32,
    image_sizes: &HashMap<String, (u32, u32)>,
) -> Vec<LayoutBox> {
    let mut boxes = Vec::new();
    let mut current_y: u32 = 16;
    let base_x: u32 = 16;
    let avail_width = viewport_width.saturating_sub(32).max(120);
    let mut inline_x: u32 = base_x;
    let mut last_inline_height: u32 = 0;

    let initial_ctx = NodeContext {
        tag: "body".to_string(),
        ..Default::default()
    };

    layout_node(
        root,
        &initial_ctx,
        rules,
        image_sizes,
        base_x,
        avail_width,
        &mut current_y,
        &mut inline_x,
        &mut last_inline_height,
        &mut boxes,
    );

    boxes
}

fn find_option_label(node: &DomNode, selected_only: bool) -> Option<String> {
    if let NodeType::Element(tag) = &node.node_type
        && tag == "option"
    {
        let text = node.get_text().trim().to_string();
        if !text.is_empty() && (!selected_only || node.get_attribute("selected").is_some()) {
            return Some(text);
        }
        return None;
    }
    for child in &node.children {
        if let Some(found) = find_option_label(child, selected_only) {
            return Some(found);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn layout_node(
    node: &DomNode,
    ctx: &NodeContext,
    rules: &[CssRule],
    image_sizes: &HashMap<String, (u32, u32)>,
    base_x: u32,
    avail_width: u32,
    current_y: &mut u32,
    inline_x: &mut u32,
    last_inline_height: &mut u32,
    boxes: &mut Vec<LayoutBox>,
) {
    match &node.node_type {
        NodeType::Element(tag_name) => {
            if matches!(
                tag_name.as_str(),
                "head" | "style" | "script" | "title" | "noscript" | "template" | "datalist" | "option"
            ) {
                return;
            }
            if node.get_attribute("hidden").is_some() {
                return;
            }

            // `<select>` açılır kutusu: Yüzlerce `<option>`'ı sayfaya dökmek yerine tek satırlık seçim kutusu çiz!
            if tag_name == "select" {
                let raw_label = find_option_label(node, true)
                    .or_else(|| find_option_label(node, false))
                    .unwrap_or_else(|| "Sec".to_string());
                let short: String = raw_label.chars().take(16).collect();
                let label = format!("{short} ▾");
                let box_h = 30u32;
                let box_w = ((label.chars().count() as u32) * 9 + 22).clamp(68, 190);

                if *inline_x + box_w > base_x + avail_width && *inline_x > base_x {
                    *current_y += (*last_inline_height).max(box_h) + 10;
                    *inline_x = base_x;
                }

                boxes.push(LayoutBox {
                    tag: "select".to_string(),
                    text: label,
                    x: *inline_x,
                    y: *current_y,
                    width: box_w,
                    height: box_h,
                    color: "#0f172a".to_string(),
                    bg_color: Some("#f8fafc".to_string()),
                    border_color: Some("#94a3b8".to_string()),
                    font_size: 14,
                    padding: 4,
                    text_align: "left".to_string(),
                    href: None,
                    image_src: None,
                    onclick: None,
                    input_name: None,
                    form_action: None,
                    is_submit: false,
                    is_password: false,
                    is_post_form: false,
                    is_multiline: false,
                    border_radius: 4,
                    has_shadow: false,
                    is_bold: false,
                    is_italic: false,
                });

                *inline_x += box_w + 10;
                *last_inline_height = (*last_inline_height).max(box_h);
                return;
            }

            // `<br>` etiketi: Açık satırı kapatıp bir alt satıra geç!
            if tag_name == "br" {
                if *inline_x > base_x {
                    *current_y += *last_inline_height + 6;
                    *inline_x = base_x;
                    *last_inline_height = 0;
                } else {
                    *current_y += 16;
                }
                return;
            }

            // `<hr>` etiketi: Boydan boya yatay ayırıcı çizgi kutusu!
            if tag_name == "hr" {
                if *inline_x > base_x {
                    *current_y += *last_inline_height + 12;
                    *inline_x = base_x;
                    *last_inline_height = 0;
                }
                boxes.push(LayoutBox {
                    tag: "hr".to_string(),
                    text: String::new(),
                    x: base_x,
                    y: *current_y + 4,
                    width: avail_width,
                    height: 2,
                    color: "#cbd5e1".to_string(),
                    bg_color: Some("#cbd5e1".to_string()),
                    border_color: None,
                    font_size: 4,
                    padding: 0,
                    text_align: "left".to_string(),
                    href: None,
                    image_src: None,
                    onclick: None,
                    input_name: None,
                    form_action: None,
                    is_submit: false,
                    is_password: false,
                    is_post_form: false,
                    is_multiline: false,
                    border_radius: 0,
                    has_shadow: false,
                    is_bold: false,
                    is_italic: false,
                });
                *current_y += 18;
                return;
            }

            // `<img src="..." alt="..." width="..." height="...">` etiketi:
            if tag_name == "img" {
                if *inline_x > base_x {
                    *current_y += *last_inline_height + 12;
                    *inline_x = base_x;
                    *last_inline_height = 0;
                }

                let src = node.get_attribute("src").unwrap_or("").trim().to_string();
                let alt = node.get_attribute("alt").unwrap_or("").trim();

                let max_img_w = avail_width.max(80);
                let attr_w = node
                    .get_attribute("width")
                    .and_then(|w| w.trim_end_matches("px").trim().parse::<u32>().ok());
                let attr_h = node
                    .get_attribute("height")
                    .and_then(|h| h.trim_end_matches("px").trim().parse::<u32>().ok());

                let (img_w, img_h) = match (attr_w, attr_h, image_sizes.get(&src)) {
                    (Some(w), Some(h), _) => (w.clamp(16, max_img_w), h.clamp(16, 480)),
                    (Some(w), None, Some(&(nat_w, nat_h))) if nat_w > 0 => {
                        let cw = w.clamp(16, max_img_w);
                        let ch = ((nat_h as u64 * cw as u64) / nat_w as u64) as u32;
                        (cw, ch.clamp(16, 480))
                    }
                    (None, None, Some(&(nat_w, nat_h))) if nat_w > 0 && nat_h > 0 => {
                        if nat_w > max_img_w {
                            let ch = ((nat_h as u64 * max_img_w as u64) / nat_w as u64) as u32;
                            (max_img_w, ch.clamp(16, 480))
                        } else {
                            (nat_w.clamp(16, max_img_w), nat_h.clamp(16, 480))
                        }
                    }
                    _ => (
                        attr_w.unwrap_or(280).clamp(16, max_img_w),
                        attr_h.unwrap_or(140).clamp(16, 480),
                    ),
                };

                let etiket_yazisi = if !alt.is_empty() {
                    format!("[Gorsel: {alt}]")
                } else if !src.is_empty() {
                    let dosya_adi = src.rsplit('/').next().unwrap_or(&src);
                    format!("[Gorsel: {dosya_adi}]")
                } else {
                    "[Gorsel]".to_string()
                };

                boxes.push(LayoutBox {
                    tag: "img".to_string(),
                    text: etiket_yazisi,
                    x: base_x,
                    y: *current_y,
                    width: img_w,
                    height: img_h,
                    color: "#475569".to_string(),
                    bg_color: Some("#f1f5f9".to_string()),
                    border_color: Some("#94a3b8".to_string()),
                    font_size: 14,
                    padding: 0,
                    text_align: "left".to_string(),
                    href: ctx.href.clone(),
                    image_src: if src.is_empty() { None } else { Some(src) },
                    onclick: ctx.onclick.clone(),
                    input_name: None,
                    form_action: None,
                    is_submit: false,
                    is_password: false,
                    is_post_form: false,
                    is_multiline: false,
                    border_radius: 0,
                    has_shadow: false,
                    is_bold: false,
                    is_italic: false,
                });

                *current_y += img_h + 14;
                return;
            }

            // `<input type="text|search|submit" name="q" placeholder="..." value="...">` etiketi:
            if tag_name == "input" {
                let input_type = node
                    .get_attribute("type")
                    .unwrap_or("text")
                    .trim()
                    .to_lowercase();
                if input_type == "hidden" {
                    return;
                }

                let is_submit = input_type == "submit" || input_type == "button";
                let is_password = input_type == "password";
                let value_attr = node.get_attribute("value").unwrap_or("").trim();
                let placeholder = node.get_attribute("placeholder").unwrap_or("").trim();
                let input_name = node
                    .get_attribute("name")
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty())
                    .or_else(|| Some("q".to_string()));

                let display_text = if !value_attr.is_empty() {
                    value_attr.to_string()
                } else if is_submit {
                    "Gonder".to_string()
                } else if !placeholder.is_empty() {
                    placeholder.to_string()
                } else {
                    "Aramak icin tiklayip yazin...".to_string()
                };

                let box_h = 30u32;
                let box_w = if is_submit {
                    ((display_text.chars().count() as u32) * 10 + 28).clamp(72, 220)
                } else {
                    (avail_width.saturating_sub(110)).clamp(160, 380)
                };

                if *inline_x + box_w > base_x + avail_width && *inline_x > base_x {
                    *current_y += (*last_inline_height).max(box_h) + 10;
                    *inline_x = base_x;
                }

                boxes.push(LayoutBox {
                    tag: if is_submit {
                        "button".to_string()
                    } else {
                        "input".to_string()
                    },
                    text: display_text,
                    x: *inline_x,
                    y: *current_y,
                    width: box_w,
                    height: box_h,
                    color: if is_submit {
                        "#ffffff".to_string()
                    } else if value_attr.is_empty() {
                        "#64748b".to_string()
                    } else {
                        "#0f172a".to_string()
                    },
                    bg_color: Some(if is_submit {
                        "#2563eb".to_string()
                    } else {
                        "#ffffff".to_string()
                    }),
                    border_color: Some(if is_submit {
                        "#1d4ed8".to_string()
                    } else {
                        "#94a3b8".to_string()
                    }),
                    font_size: 15,
                    padding: 4,
                    text_align: if is_submit {
                        "center".to_string()
                    } else {
                        "left".to_string()
                    },
                    href: None,
                    image_src: None,
                    onclick: node.get_attribute("onclick").map(str::to_string),
                    input_name: if is_submit { None } else { input_name },
                    form_action: ctx.form_action.clone(),
                    is_submit,
                    is_password,
                    is_post_form: ctx.form_method_post,
                    is_multiline: false,
                    border_radius: 5,
                    has_shadow: is_submit,
                    is_bold: is_submit,
                    is_italic: false,
                });

                *inline_x += box_w + 10;
                *last_inline_height = (*last_inline_height).max(box_h);
                return;
            }

            // Çok satırlı metin alanı (`<textarea name="..." rows="3" placeholder="...">`):
            if tag_name == "textarea" {
                if *inline_x > base_x {
                    *current_y += *last_inline_height + 10;
                    *inline_x = base_x;
                    *last_inline_height = 0;
                }

                let rows = node
                    .get_attribute("rows")
                    .and_then(|r| r.trim().parse::<u32>().ok())
                    .unwrap_or(3)
                    .clamp(2, 10);
                let inner_default: String = node
                    .children
                    .iter()
                    .filter_map(|c| match &c.node_type {
                        NodeType::Text(t) => Some(t.as_str()),
                        _ => None,
                    })
                    .collect();
                let placeholder = node.get_attribute("placeholder").unwrap_or("").trim();
                let input_name = node
                    .get_attribute("name")
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty())
                    .or_else(|| Some("mesaj".to_string()));
                let display_text = if !inner_default.trim().is_empty() {
                    inner_default.trim().to_string()
                } else if !placeholder.is_empty() {
                    placeholder.to_string()
                } else {
                    "Cok satirli metin yazmak icin tiklayin...".to_string()
                };

                let box_h = rows * 20 + 14;
                let box_w = (avail_width.saturating_sub(60)).clamp(200, 460);

                boxes.push(LayoutBox {
                    tag: "textarea".to_string(),
                    text: display_text,
                    x: base_x,
                    y: *current_y,
                    width: box_w,
                    height: box_h,
                    color: "#64748b".to_string(),
                    bg_color: Some("#ffffff".to_string()),
                    border_color: Some("#94a3b8".to_string()),
                    font_size: 15,
                    padding: 6,
                    text_align: "left".to_string(),
                    href: None,
                    image_src: None,
                    onclick: None,
                    input_name,
                    form_action: ctx.form_action.clone(),
                    is_submit: false,
                    is_password: false,
                    is_post_form: ctx.form_method_post,
                    is_multiline: true,
                    border_radius: 6,
                    has_shadow: false,
                    is_bold: false,
                    is_italic: false,
                });

                *current_y += box_h + 12;
                return;
            }

            // Blok etiketine girerken açıkta kalan yan yana (inline) satırı kapat:
            let is_inline_tag = matches!(
                tag_name.as_str(),
                "a" | "span" | "button" | "b" | "strong" | "i" | "em" | "code" | "small"
            );
            if !is_inline_tag && *inline_x > base_x {
                *current_y += *last_inline_height + 12;
                *inline_x = base_x;
                *last_inline_height = 0;
            }

            // Elementin kendi doğrudan CSS özelliklerini (`display: flex`, `width`, `max-width`, `padding`, `margin`) çöz:
            let (own_id, own_classes, own_inline) = extract_own_attrs(node);
            let mut own_style = resolve_element_style(
                tag_name,
                own_id.as_deref(),
                &own_classes,
                &ctx.ancestors,
                &own_inline,
                rules,
            );
            if own_style.display == "none" {
                return;
            }

            if own_style.color == "black" {
                if let Some(parent_color) = &ctx.inherited_color {
                    own_style.color = parent_color.clone();
                } else if ctx.href.is_some() {
                    own_style.color = "#1d4ed8".to_string();
                }
            }
            if own_style.text_align == "left"
                && let Some(parent_align) = &ctx.text_align
            {
                own_style.text_align = parent_align.clone();
            }

            let box_avail_w = effective_box_width(avail_width, &own_style);

            // Ebeveyn bağlamını miras alarak çocuklara aktarılacak `next_ctx` oluştur:
            let mut next_ctx = ctx.clone();
            next_ctx.tag = tag_name.clone();
            next_ctx.ancestors.push(AncestorInfo {
                tag: tag_name.clone(),
                id: own_id,
                classes: own_classes,
            });
            if let Some(href_val) = node.get_attribute("href") {
                next_ctx.href = Some(href_val.to_string());
                if own_style.color == "black" {
                    own_style.color = "#1d4ed8".to_string();
                }
            }
            if let Some(onclick_val) = node.get_attribute("onclick") {
                next_ctx.onclick = Some(onclick_val.to_string());
            }
            if tag_name == "form" {
                let action = node.get_attribute("action").unwrap_or("").trim().to_string();
                next_ctx.form_action = Some(action);
                next_ctx.form_method_post = node
                    .get_attribute("method")
                    .is_some_and(|m| m.trim().eq_ignore_ascii_case("post"));
            }
            if own_style.text_align != "left" {
                next_ctx.text_align = Some(own_style.text_align.clone());
            }
            if own_style.color != "black" {
                next_ctx.inherited_color = Some(own_style.color.clone());
            }
            if own_style.font_bold.is_none() {
                own_style.font_bold = ctx.inherited_bold;
            }
            if own_style.font_italic.is_none() {
                own_style.font_italic = ctx.inherited_italic;
            }
            if own_style.font_bold.is_some()
                || matches!(tag_name.as_str(), "b" | "strong" | "h1" | "h2" | "h3")
            {
                next_ctx.inherited_bold = Some(
                    own_style
                        .font_bold
                        .unwrap_or(matches!(tag_name.as_str(), "b" | "strong" | "h1" | "h2" | "h3")),
                );
            }
            if own_style.font_italic.is_some() || matches!(tag_name.as_str(), "em" | "i") {
                next_ctx.inherited_italic = Some(
                    own_style
                        .font_italic
                        .unwrap_or(matches!(tag_name.as_str(), "em" | "i")),
                );
            }

            // Dış boşluk (margin-top) uygula:
            if !is_inline_tag && own_style.margin > 0 {
                *current_y += own_style.margin;
            }

            // =================================================================
            // 1. CSS FLEXBOX (`display: flex`) ve HTML TABLO SATIRI (`<tr>`) YERLEŞİMİ!
            // =================================================================
            let is_table_row = tag_name == "tr";
            if own_style.display == "flex" || is_table_row {
                let flex_items: Vec<&DomNode> = node
                    .children
                    .iter()
                    .filter(|ch| matches!(ch.node_type, NodeType::Element(_)))
                    .collect();

                if !flex_items.is_empty() {
                    let count = flex_items.len() as u32;
                    let gap = if is_table_row { 8 } else { own_style.gap };
                    let total_gaps = gap.saturating_mul(count.saturating_sub(1));
                    let col_width = box_avail_w.saturating_sub(total_gaps) / count;
                    let start_y = *current_y;
                    let mut max_bottom_y = start_y;
                    next_ctx.current_style = own_style.clone();

                    for (idx, item) in flex_items.iter().enumerate() {
                        let col_x = base_x + (idx as u32) * (col_width + gap);
                        let mut col_y = start_y;
                        let mut col_inline_x = col_x;
                        let mut col_inline_h = 0;

                        layout_node(
                            item,
                            &next_ctx,
                            rules,
                            image_sizes,
                            col_x,
                            col_width.max(50),
                            &mut col_y,
                            &mut col_inline_x,
                            &mut col_inline_h,
                            boxes,
                        );
                        if col_inline_x > col_x {
                            col_y += col_inline_h + 6;
                        }
                        if col_y > max_bottom_y {
                            max_bottom_y = col_y;
                        }
                    }

                    *current_y = max_bottom_y + own_style.margin;
                    return;
                }
            }

            // =================================================================
            // 2. KAPSAYICI KART / KUTU (Container Box):
            // Eğer `<div>`, `<section>`, `<article>` gibi bir kapsayıcı arka plan (`bg_color`)
            // veya çerçeve (`border_color`) aldıysa, içindeki tüm çok-satırlı metinleri
            // TEK BİR BÜTÜN KART ÇERÇEVESİ içinde çiz!
            // =================================================================
            let is_box_container = matches!(
                tag_name.as_str(),
                "div" | "section" | "article" | "header" | "footer" | "main"
            ) && (own_style.bg_color.is_some()
                || own_style.border_color.is_some());

            if is_box_container {
                let pad = own_style.padding.max(8);
                let box_start_y = *current_y;
                // Önce kapsayıcı arka plan kutusunu `boxes` listesine yer tutucu olarak ekle
                // (böylece çocuklar çizildikten sonra yüksekliği kesinleşir ve metinlerin ARKASINDA kalır!)
                let container_idx = boxes.len();
                boxes.push(LayoutBox {
                    tag: tag_name.clone(),
                    text: String::new(),
                    x: base_x,
                    y: box_start_y,
                    width: box_avail_w,
                    height: 0,
                    color: own_style.color.clone(),
                    bg_color: own_style.bg_color.clone(),
                    border_color: own_style.border_color.clone(),
                    font_size: own_style.font_size,
                    padding: pad,
                    text_align: own_style.text_align.clone(),
                    href: None,
                    image_src: None,
                    onclick: None,
                    input_name: None,
                    form_action: None,
                    is_submit: false,
                    is_password: false,
                    is_post_form: false,
                    is_multiline: false,
                    border_radius: own_style.border_radius,
                    has_shadow: own_style.has_shadow,
                    is_bold: own_style.font_bold.unwrap_or(false),
                    is_italic: own_style.font_italic.unwrap_or(false),
                });

                next_ctx.container_painted_bg = true;
                next_ctx.current_style = own_style.clone();
                *current_y += pad;
                let inner_x = base_x + pad;
                let inner_w = box_avail_w.saturating_sub(pad * 2).max(80);
                let mut inner_inline_x = inner_x;
                let mut inner_inline_h = 0;

                for child in &node.children {
                    layout_node(
                        child,
                        &next_ctx,
                        rules,
                        image_sizes,
                        inner_x,
                        inner_w,
                        current_y,
                        &mut inner_inline_x,
                        &mut inner_inline_h,
                        boxes,
                    );
                }

                if inner_inline_x > inner_x {
                    *current_y += inner_inline_h + 8;
                }

                // Kapsayıcının toplam yüksekliğini içindeki çocuklara göre tam ayarla:
                let content_end_y = (*current_y).saturating_sub(4);
                let total_h = content_end_y
                    .saturating_sub(box_start_y)
                    .saturating_add(pad)
                    .max(pad * 2 + 16);
                boxes[container_idx].height = total_h;
                *current_y = box_start_y + total_h + own_style.margin.max(12);
                return;
            }

            next_ctx.current_style = own_style.clone();

            // Normal blok / inline element çocuklarını gez:
            for child in &node.children {
                layout_node(
                    child,
                    &next_ctx,
                    rules,
                    image_sizes,
                    base_x,
                    box_avail_w,
                    current_y,
                    inline_x,
                    last_inline_height,
                    boxes,
                );
            }

            if !is_inline_tag && *inline_x > base_x {
                *current_y += *last_inline_height + 12;
                *inline_x = base_x;
                *last_inline_height = 0;
            }
            if !is_inline_tag && own_style.margin > 0 {
                *current_y += own_style.margin;
            }
        }

        NodeType::Text(text) => {
            let current_tag = ctx.tag.as_str();
            let mut style = ctx.current_style.clone();

            if ctx.href.is_some() && style.color == "black" {
                style.color = "#1d4ed8".to_string();
            }

            // Ebeveyn `.kart` / `<div>` zaten kart arka planını çizdiyse, içindeki düz metin
            // satırlarının her birine tekrar tekrar çerçeve/arka plan basma!
            if ctx.container_painted_bg && matches!(current_tag, "p" | "li" | "div" | "span") {
                style.bg_color = None;
                style.border_color = None;
                style.has_shadow = false;
            }
            let text_is_bold = style
                .font_bold
                .unwrap_or(matches!(current_tag, "h1" | "h2" | "h3" | "strong" | "b"));
            let text_is_italic = style
                .font_italic
                .unwrap_or(matches!(current_tag, "em" | "i"));

            let pad = style.padding;
            let line_height = style.font_size + 10 + pad * 2;
            // Cairo ve Win32 orantılı font genişliği için güvenli karakter genişliği tahmini:
            let char_w = ((style.font_size * 64) / 100).max(8);

            // YAN YANA (INLINE) DİZİLİM:
            // Kısa `<a>`, `<span>`, `<button>`, `<strong>`, `<b>`, `<em>`, `<i>`, `<code>`, `<small>` öğeleri satıra sığıyorsa yan yana diz!
            let is_short_inline = matches!(
                current_tag,
                "a" | "span" | "button" | "strong" | "b" | "em" | "i" | "code" | "small"
            ) && text.chars().count() <= 28;

            if is_short_inline {
                let extra_w = if matches!(current_tag, "a" | "button") {
                    26
                } else {
                    8
                };
                let item_w = (text.chars().count() as u32 * char_w) + extra_w + pad * 2;
                if *inline_x + item_w > base_x + avail_width && *inline_x > base_x {
                    *current_y += (*last_inline_height).max(line_height) + 10;
                    *inline_x = base_x;
                }

                boxes.push(LayoutBox {
                    tag: current_tag.to_string(),
                    text: text.clone(),
                    x: *inline_x,
                    y: *current_y,
                    width: item_w,
                    height: line_height,
                    color: style.color,
                    bg_color: style.bg_color,
                    border_color: style.border_color,
                    font_size: style.font_size,
                    padding: pad,
                    text_align: style.text_align,
                    href: ctx.href.clone(),
                    image_src: None,
                    onclick: ctx.onclick.clone(),
                    input_name: None,
                    form_action: if current_tag == "button" {
                        ctx.form_action.clone()
                    } else {
                        None
                    },
                    is_submit: current_tag == "button"
                        && ctx.form_action.is_some()
                        && ctx.onclick.is_none(),
                    is_password: false,
                    is_post_form: ctx.form_method_post,
                    is_multiline: false,
                    border_radius: style.border_radius,
                    has_shadow: style.has_shadow,
                    is_bold: text_is_bold,
                    is_italic: text_is_italic,
                });

                *inline_x += item_w + 12;
                *last_inline_height = (*last_inline_height).max(line_height);
                return;
            }

            // Normal blok kutuya geçmeden önce açık inline satırı varsa kapat:
            if *inline_x > base_x {
                *current_y += *last_inline_height + 12;
                *inline_x = base_x;
                *last_inline_height = 0;
            }

            let is_list_item = current_tag == "li";
            let x = if is_list_item { base_x + 16 } else { base_x };
            let width = if is_list_item {
                avail_width.saturating_sub(16).max(60)
            } else {
                avail_width
            };

            let formatted_text = if is_list_item {
                format!("• {text}")
            } else {
                text.clone()
            };

            let inner_wrap_w = width.saturating_sub(pad * 2 + 12).max(60);
            let lines = wrap_text(&formatted_text, inner_wrap_w, style.font_size);
            let line_count = lines.len();

            // Eğer çok satırlı bir paragrafın kendi bg/border'ı varsa onu da tek bir arka plan kutusunda birleştir:
            let has_own_frame =
                line_count > 1 && (style.bg_color.is_some() || style.border_color.is_some());
            if has_own_frame {
                let single_h = style.font_size + 10;
                let total_frame_h =
                    (line_count as u32) * (single_h + 4) + pad * 2;
                boxes.push(LayoutBox {
                    tag: current_tag.to_string(),
                    text: String::new(),
                    x,
                    y: *current_y,
                    width,
                    height: total_frame_h,
                    color: style.color.clone(),
                    bg_color: style.bg_color.clone(),
                    border_color: style.border_color.clone(),
                    font_size: style.font_size,
                    padding: pad,
                    text_align: style.text_align.clone(),
                    href: None,
                    image_src: None,
                    onclick: None,
                    input_name: None,
                    form_action: None,
                    is_submit: false,
                    is_password: false,
                    is_post_form: false,
                    is_multiline: false,
                    border_radius: style.border_radius,
                    has_shadow: style.has_shadow,
                    is_bold: text_is_bold,
                    is_italic: text_is_italic,
                });
                *current_y += pad;
            }

            for (idx, line) in lines.into_iter().enumerate() {
                let y = *current_y;
                let h = if has_own_frame {
                    style.font_size + 10
                } else {
                    line_height
                };
                boxes.push(LayoutBox {
                    tag: current_tag.to_string(),
                    text: line,
                    x: if has_own_frame { x + pad } else { x },
                    y,
                    width: if has_own_frame {
                        width.saturating_sub(pad * 2)
                    } else {
                        width
                    },
                    height: h,
                    color: style.color.clone(),
                    bg_color: if has_own_frame {
                        None
                    } else {
                        style.bg_color.clone()
                    },
                    border_color: if has_own_frame {
                        None
                    } else {
                        style.border_color.clone()
                    },
                    font_size: style.font_size,
                    padding: if has_own_frame { 0 } else { pad },
                    text_align: style.text_align.clone(),
                    href: ctx.href.clone(),
                    image_src: None,
                    onclick: ctx.onclick.clone(),
                    input_name: None,
                    form_action: None,
                    is_submit: false,
                    is_password: false,
                    is_post_form: false,
                    is_multiline: false,
                    border_radius: if has_own_frame { 0 } else { style.border_radius },
                    has_shadow: if has_own_frame { false } else { style.has_shadow },
                    is_bold: text_is_bold,
                    is_italic: text_is_italic,
                });

                if idx + 1 < line_count {
                    *current_y += h + 4;
                } else {
                    *current_y += h + if has_own_frame { pad + 12 } else { 12 };
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

        // 2 menü linki + 1 birleşik `.kart` çerçeve kutusu + 1 içindeki `<p>` metin kutusu = 4 kutu
        assert_eq!(kutular.len(), 4);
        // İlk iki menü linki aynı satırda (y = 16) yan yana (farklı x) durmalı!
        assert_eq!(kutular[0].y, 16);
        assert_eq!(kutular[1].y, 16);
        assert!(kutular[1].x > kutular[0].x);

        // 3. kutu kapsayıcı `.kart` kutusu (arka plan ve çerçeve tek bir kutuda birleşik!)
        assert_eq!(kutular[2].bg_color, Some("#e0f2fe".to_string()));
        assert_eq!(kutular[2].border_color, Some("#0284c7".to_string()));
        // 4. kutu içindeki metin (#ozel rengini miras almış ve tekrar çerçeve çizmiyor!)
        assert_eq!(kutular[3].color, "#0369a1");
        assert_eq!(kutular[3].border_color, None);
    }

    #[test]
    fn test_img_etiketi_kutu_yerlesimi() {
        let html = "<body><h1>Galeri</h1><img src=\"/logo.bmp\" alt=\"Anka Logosu\" width=\"320\" height=\"120\"><p>Alt yazi</p></body>";
        let dom = parse_html(html);
        let kutular = compute_layout(&dom, &[], 800);

        assert_eq!(kutular.len(), 3);
        let img = &kutular[1];
        assert_eq!(img.tag, "img");
        assert_eq!(img.image_src.as_deref(), Some("/logo.bmp"));
        assert_eq!(img.text, "[Gorsel: Anka Logosu]");
        assert_eq!(img.width, 320);
        assert_eq!(img.height, 120);
        assert!(kutular[2].y >= img.y + img.height);
    }

    #[test]
    fn test_flexbox_yanyana_sutunlar_ve_kutu_modeli() {
        let html = "<body><div class=\"satir\"><div class=\"kutu\"><p>Sutun 1</p></div><div class=\"kutu\"><p>Sutun 2</p></div></div></body>";
        let css = ".satir { display: flex; gap: 16px; } .kutu { background-color: #f8fafc; border: 1px solid #cbd5e1; padding: 10px; text-align: center; }";

        let dom = parse_html(html);
        let kurallar = parse_css(css);
        let kutular = compute_layout(&dom, &kurallar, 800);

        // 2 kapsayıcı kutu (Sutun 1 ve Sutun 2) aynı `y` yüksekliğinde, yan yana (`x` farklı) dizilmeli!
        let kartlar: Vec<&LayoutBox> = kutular.iter().filter(|b| b.tag == "div").collect();
        assert_eq!(kartlar.len(), 2);
        assert_eq!(kartlar[0].y, kartlar[1].y);
        assert!(kartlar[1].x > kartlar[0].x + kartlar[0].width);
        assert_eq!(kartlar[0].padding, 10);
        assert_eq!(kartlar[0].text_align, "center");
    }

    #[test]
    fn test_form_ve_input_yerlesimi() {
        let html = "<body><form action=\"https://tr.wikipedia.org/w/index.php\"><input type=\"text\" name=\"search\" placeholder=\"Vikipedi ara\"><input type=\"submit\" value=\"Ara\"></form></body>";
        let dom = parse_html(html);
        let kutular = compute_layout(&dom, &[], 800);

        assert_eq!(kutular.len(), 2);
        assert_eq!(kutular[0].tag, "input");
        assert_eq!(kutular[0].input_name.as_deref(), Some("search"));
        assert_eq!(
            kutular[0].form_action.as_deref(),
            Some("https://tr.wikipedia.org/w/index.php")
        );
        assert!(!kutular[0].is_submit);

        assert_eq!(kutular[1].tag, "button");
        assert!(kutular[1].is_submit);
        // Metin kutusu ile Gönder butonu aynı satırda yan yana olmalı!
        assert_eq!(kutular[0].y, kutular[1].y);
        assert!(kutular[1].x > kutular[0].x);
    }

    #[test]
    fn test_display_none_hidden_br_ve_inline_etiketler() {
        let html = "<body><div style=\"display: none\"><p>Gizli CSS</p></div><p hidden>Gizli Attr</p><noscript><p>Noscript</p></noscript><p>Satir 1</p><br><small>Kucuk Not</small><strong>Kalın Baslik</strong></body>";
        let dom = parse_html(html);
        let kutular = compute_layout(&dom, &[], 800);

        assert!(!kutular.iter().any(|b| b.text.contains("Gizli")));
        assert!(!kutular.iter().any(|b| b.text.contains("Noscript")));
        let small = kutular.iter().find(|b| b.tag == "small").expect("small bulunamadi");
        let strong = kutular.iter().find(|b| b.tag == "strong").expect("strong bulunamadi");
        assert_eq!(small.font_size, 13);
        assert!(small.y > kutular[0].y + kutular[0].height);
        assert_eq!(strong.text, "Kalın Baslik");
    }

    #[test]
    fn test_table_tr_td_yatay_yerlesim() {
        let html = "<body><table><tr><td>Sutun A</td><td>Sutun B</td><td>Sutun C</td></tr></table></body>";
        let dom = parse_html(html);
        let kutular = compute_layout(&dom, &[], 800);

        assert_eq!(kutular.len(), 3);
        assert_eq!(kutular[0].y, kutular[1].y);
        assert_eq!(kutular[1].y, kutular[2].y);
        assert!(kutular[1].x > kutular[0].x);
        assert!(kutular[2].x > kutular[1].x);
    }

    #[test]
    fn test_body_arka_plani_ve_orantisal_gorsel_boyutu() {
        let html = "<body bgcolor=\"#f6f6ef\"><img src=\"/hero.png\" alt=\"Hero\"></body>";
        let dom = parse_html(html);
        let bg = resolve_body_background(&dom, &[]);
        assert_eq!(bg.as_deref(), Some("#f6f6ef"));

        let mut sizes = HashMap::new();
        sizes.insert("/hero.png".to_string(), (800, 400));
        let kutular = compute_layout_with_images(&dom, &[], 500, &sizes);
        assert_eq!(kutular.len(), 1);
        // 500px viewport -> max_w = 468, oranti 2:1 -> height ~ 234
        assert!(kutular[0].width <= 468);
        assert_eq!(kutular[0].height, (kutular[0].width * 400) / 800);
    }

    #[test]
    fn test_select_option_ve_link_alt_etiket_rengi() {
        let html = "<body><select><option>Turkce</option><option selected>English</option></select><a href=\"/en\"><strong>English</strong></a></body>";
        let dom = parse_html(html);
        let kutular = compute_layout(&dom, &[], 800);

        assert_eq!(kutular.len(), 2);
        assert_eq!(kutular[0].tag, "select");
        assert!(kutular[0].text.starts_with("English"));
        assert_eq!(kutular[1].tag, "strong");
        assert_eq!(kutular[1].href.as_deref(), Some("/en"));
        assert_eq!(kutular[1].color, "#1d4ed8");
    }

    #[test]
    fn test_css_soy_cocuk_secicileri_ve_genislik() {
        let html = "<body><div class=\"menu\"><a href=\"/1\">Menu Link</a></div><div class=\"kart\"><div><p>Torun</p></div><p>Cocuk</p></div></body>";
        let css = ".menu a { color: red; } .kart > p { color: green; } .kart { background-color: #f8fafc; max-width: 300px; }";
        let dom = parse_html(html);
        let kurallar = parse_css(css);
        let kutular = compute_layout(&dom, &kurallar, 800);

        let menu_link = kutular.iter().find(|b| b.text == "Menu Link").unwrap();
        let torun = kutular.iter().find(|b| b.text == "Torun").unwrap();
        let cocuk = kutular.iter().find(|b| b.text == "Cocuk").unwrap();
        let kart = kutular.iter().find(|b| b.tag == "div" && b.bg_color.is_some()).unwrap();

        assert_eq!(menu_link.color, "red");
        assert_eq!(torun.color, "black");
        assert_eq!(cocuk.color, "green");
        assert_eq!(kart.width, 300);
    }

    #[test]
    fn test_post_form_ve_parola_input_ozellikleri() {
        let html = "<body><form action=\"/oturum\" method=\"post\"><input type=\"text\" name=\"user\"><input type=\"password\" name=\"pw\"><input type=\"submit\" value=\"Giris\"></form></body>";
        let dom = parse_html(html);
        let kutular = compute_layout(&dom, &[], 800);

        let user_box = kutular
            .iter()
            .find(|b| b.input_name.as_deref() == Some("user"))
            .expect("user input kutusu");
        let pw_box = kutular
            .iter()
            .find(|b| b.input_name.as_deref() == Some("pw"))
            .expect("password input kutusu");
        let submit_box = kutular
            .iter()
            .find(|b| b.is_submit)
            .expect("submit butonu");

        assert!(!user_box.is_password);
        assert!(user_box.is_post_form);
        assert!(pw_box.is_password);
        assert!(pw_box.is_post_form);
        assert!(submit_box.is_post_form);
        assert_eq!(submit_box.form_action.as_deref(), Some("/oturum"));
    }

    #[test]
    fn test_css_border_radius_ayristirma() {
        let html = "<body><div class=\"yuvarlak\"><p>Yuvarlak Kose</p></div><a class=\"btn\" href=\"/ok\">Buton</a></body>";
        let css = ".yuvarlak { background-color: #f8fafc; border-radius: 12px; padding: 8px; } .btn { background-color: #dbeafe; border-radius: 6px; }";
        let dom = parse_html(html);
        let kurallar = parse_css(css);
        let kutular = compute_layout(&dom, &kurallar, 800);

        let kart = kutular
            .iter()
            .find(|b| b.tag == "div" && b.bg_color.is_some())
            .expect("yuvarlak div");
        let btn = kutular.iter().find(|b| b.tag == "a").expect("btn link");

        assert_eq!(kart.border_radius, 12);
        assert_eq!(btn.border_radius, 6);
    }

    #[test]
    fn test_textarea_cok_satirli_yerlesim() {
        let html = "<body><form action=\"/not\" method=\"POST\"><textarea name=\"icerik\" rows=\"4\" placeholder=\"Not yaz...\"></textarea><input type=\"submit\" value=\"Gonder\"></form></body>";
        let dom = parse_html(html);
        let kutular = compute_layout(&dom, &[], 800);

        let area = kutular
            .iter()
            .find(|b| b.tag == "textarea")
            .expect("textarea kutusu");
        assert!(area.is_multiline);
        assert!(area.is_post_form);
        assert_eq!(area.input_name.as_deref(), Some("icerik"));
        assert_eq!(area.text, "Not yaz...");
        assert_eq!(area.height, 94);
    }

    #[test]
    fn test_box_shadow_ve_font_weight_style_mirasi() {
        let html = "<body><div class=\"kart\"><p>Kalin ve Egik</p><p class=\"duz\">Normal</p></div></body>";
        let css = ".kart { background-color: #ecfdf5; box-shadow: 0 4px 10px rgba(0,0,0,0.1); font-weight: bold; font-style: italic; } .duz { font-weight: normal; font-style: normal; }";
        let dom = parse_html(html);
        let kurallar = parse_css(css);
        let kutular = compute_layout(&dom, &kurallar, 800);

        let kart = kutular
            .iter()
            .find(|b| b.tag == "div" && b.bg_color.is_some())
            .expect("golgeli kart kutusu");
        let p_miras = kutular
            .iter()
            .find(|b| b.text == "Kalin ve Egik")
            .expect("miras alan p");
        let p_duz = kutular.iter().find(|b| b.text == "Normal").expect("normal p");

        assert!(kart.has_shadow);
        assert!(!p_miras.has_shadow);
        assert!(p_miras.is_bold);
        assert!(p_miras.is_italic);
        assert!(!p_duz.is_bold);
        assert!(!p_duz.is_italic);
    }
}
