// Anka Browser - HTML Ayrıştırıcı ve DOM Ağacı (İstasyon 3)

/// Bir DOM düğümünün (kutunun) türü:
/// HTML'de her şey ya bir Etikettir (Element) VEYA düz bir Yazıdır (Text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    // Etiket adı (örneğin: "html", "body", "h1", "p")
    Element(String),

    // Etiketin içindeki düz metin (örneğin: "Merhaba Anka!")
    Text(String),
}

/// DOM Ağacındaki her bir kutuyu (düğümü) temsil eden paket:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomNode {
    // Bu kutu bir etiket mi yoksa düz yazı mı?
    pub node_type: NodeType,

    // Etiket özellikleri (örneğin: [("href", "https://example.com")])
    pub attributes: Vec<(String, String)>,

    // Bu kutunun içinde bulunan alt kutuların (çocukların) listesi
    pub children: Vec<DomNode>,
}

impl DomNode {
    /// Yeni bir Etiket (Element) kutusu oluşturmak için kısayol fonksiyonu
    pub fn element(tag_name: &str) -> Self {
        DomNode {
            node_type: NodeType::Element(tag_name.to_lowercase()),
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Özellikleriyle birlikte yeni bir Etiket (Element) kutusu oluşturur
    pub fn element_with_attrs(tag_name: &str, attributes: Vec<(String, String)>) -> Self {
        DomNode {
            node_type: NodeType::Element(tag_name.to_lowercase()),
            attributes,
            children: Vec::new(),
        }
    }

    /// Yeni bir Düz Yazı (Text) kutusu oluşturmak için kısayol fonksiyonu
    /// İçindeki `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&nbsp;` gibi HTML karakter kodlarını otomatik çözer.
    pub fn text(content: &str) -> Self {
        DomNode {
            node_type: NodeType::Text(decode_html_entities(content)),
            attributes: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Etiketin belirli bir özelliğinin (örn: "href") değerini döner
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Etiketin belirli bir özelliğini (örn: `"style"`) günceller veya yoksa yeni ekler.
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        if let Some((_, v)) = self
            .attributes
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
        {
            *v = value.to_string();
        } else {
            self.attributes.push((name.to_lowercase(), value.to_string()));
        }
    }

    /// Bu düğümün tüm çocuklarını temizleyip içine tek bir düz yazı (`NodeType::Text`) yerleştirir.
    /// JavaScript'teki `element.innerText = "..."` / `textContent = "..."` işleminin karşılığıdır!
    pub fn set_text(&mut self, new_text: &str) {
        self.children.clear();
        self.children.push(DomNode::text(new_text));
    }

    /// Bu düğümün içindeki düz metni okur (JavaScript `element.innerText` okuması için).
    pub fn get_text(&self) -> String {
        let mut out = String::new();
        for child in &self.children {
            match &child.node_type {
                NodeType::Text(t) => out.push_str(t),
                NodeType::Element(_) => out.push_str(&child.get_text()),
            }
        }
        out
    }

    /// DOM ağacında `id="..."` özelliği verilen kimlikle eşleşen düğümü değiştirilebilir (`&mut`)
    /// olarak bulur (`document.getElementById("...")` temel taşı!).
    pub fn find_by_id_mut(&mut self, target_id: &str) -> Option<&mut DomNode> {
        if let NodeType::Element(_) = &self.node_type
            && self
                .get_attribute("id")
                .is_some_and(|v| v.trim() == target_id)
        {
            return Some(self);
        }

        for child in &mut self.children {
            if let Some(found) = child.find_by_id_mut(target_id) {
                return Some(found);
            }
        }
        None
    }
}

/// Etiketin içindeki özellikleri (örn: `a href="Status.html"`) ayrıştırır
fn parse_tag_and_attrs(tag_content: &str) -> (String, Vec<(String, String)>) {
    let mut parts = tag_content.splitn(2, char::is_whitespace);
    let tag_name = parts.next().unwrap_or("").to_lowercase();
    let mut attrs = Vec::new();

    if let Some(mut rest) = parts.next() {
        while !rest.is_empty() {
            rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            if let Some(eq_idx) = rest.find('=') {
                let key = rest[..eq_idx].trim().to_lowercase();
                let after_eq = rest[eq_idx + 1..].trim_start();
                if let Some(quote) = after_eq.chars().next().filter(|&c| c == '"' || c == '\'') {
                    let after_quote = &after_eq[1..];
                    if let Some(end_quote) = after_quote.find(quote) {
                        let val = after_quote[..end_quote].to_string();
                        if !key.is_empty() {
                            attrs.push((key, val));
                        }
                        rest = &after_quote[end_quote + 1..];
                        continue;
                    }
                }
                let end_val = after_eq.find(char::is_whitespace).unwrap_or(after_eq.len());
                let val = after_eq[..end_val].to_string();
                if !key.is_empty() {
                    attrs.push((key, val));
                }
                rest = &after_eq[end_val..];
            } else {
                break;
            }
        }
    }

    (tag_name, attrs)
}

/// Düz HTML metnini okur ve iç içe kutulardan oluşan DOM Ağacına (`DomNode`) dönüştürür.
/// Örnek girdi: "<body><h1>Merhaba</h1><p>Anka</p></body>"
pub fn parse_html(raw: &str) -> DomNode {
    // Masanın üstündeki "Açık Kutular Yığını" (Stack).
    // En altta tüm sayfayı tutan bir "document" (belge) kutusuyla başlıyoruz.
    let mut stack: Vec<DomNode> = vec![DomNode::element("document")];
    let mut kalan = raw;

    while !kalan.is_empty() {
        // 1. DURUM: Metinde '<' (etiket başlangıcı) var mı?
        if let Some(tag_start) = kalan.find('<') {
            // '<' işaretinden önce düz bir yazı varsa (örn: "Merhaba"),
            // onu şu an açık olan en üstteki kutunun içine (children) ekle!
            let onceki_yazi = kalan[..tag_start].trim();
            if !onceki_yazi.is_empty()
                && let Some(acik_kutu) = stack.last_mut()
            {
                acik_kutu.children.push(DomNode::text(onceki_yazi));
            }

            // Şimdi '<' ile '>' arasındaki etiket kısmını bulalım
            kalan = &kalan[tag_start + 1..];
            let Some(tag_end) = kalan.find('>') else {
                // Eğer '>' kapatılmamışsa (bozuk HTML), döngüyü güvenle bitir
                break;
            };

            let tag_icerigi = kalan[..tag_end].trim();
            kalan = &kalan[tag_end + 1..];

            // Yorum satırı (<!-- ... -->) veya <!DOCTYPE html> ise atla
            if tag_icerigi.starts_with('!') || tag_icerigi.starts_with('?') {
                continue;
            }

            // 2. DURUM: Kapanış Etiketi mi? (Başında '/' varsa, örn: "</h1>" veya "</body>")
            if tag_icerigi.starts_with('/') {
                // Masanın üstünde kapatılacak bir kutu varsa (en alttaki document hariç):
                // Kutuyu yığından al (pop) ve bir alttaki ebeveyn kutunun çocuklarına ekle!
                if stack.len() > 1 {
                    let biten_kutu = stack.pop().unwrap();
                    if let Some(ebeveyn_kutu) = stack.last_mut() {
                        ebeveyn_kutu.children.push(biten_kutu);
                    }
                }
            } else {
                // 3. DURUM: Açılış Etiketi! (Örn: "h1", "body", "p class='ozel'", "br/")
                let kendi_kapanan_mi = tag_icerigi.ends_with('/');
                let temiz_etiket = tag_icerigi.trim_end_matches('/');

                // Etiketin adını ve varsa özelliklerini (örn: href="...") ayrıştır
                let (etiket_adi, ozellikler) = parse_tag_and_attrs(temiz_etiket);

                if etiket_adi.is_empty() {
                    continue;
                }

                // Kendi kendine kapanan (içine çocuk almayan) etiketler: <br>, <img>, <meta> vb.
                let tekil_etiket = matches!(
                    etiket_adi.as_str(),
                    "br" | "hr" | "img" | "meta" | "link" | "input"
                );

                if kendi_kapanan_mi || tekil_etiket {
                    if let Some(acik_kutu) = stack.last_mut() {
                        acik_kutu
                            .children
                            .push(DomNode::element_with_attrs(&etiket_adi, ozellikler));
                    }
                } else {
                    // ÖZEL RAW TEXT KURALI (`<script>` ve `<style>`):
                    // JavaScript içindeki `if (x < 5)` gibi `<` işaretleri HTML etiketi sanılmasın diye,
                    // `</script>` kapanış etiketini görene kadar tüm ham kodu tek parça metin olarak al!
                    if etiket_adi == "script" {
                        let mut script_node =
                            DomNode::element_with_attrs(&etiket_adi, ozellikler);
                        let lower_kalan = kalan.to_lowercase();
                        if let Some(end_idx) = lower_kalan.find("</script>") {
                            let js_kod = kalan[..end_idx].trim();
                            if !js_kod.is_empty() {
                                script_node.children.push(DomNode {
                                    node_type: NodeType::Text(js_kod.to_string()),
                                    attributes: Vec::new(),
                                    children: Vec::new(),
                                });
                            }
                            kalan = &kalan[end_idx + "</script>".len()..];
                        } else {
                            let js_kod = kalan.trim();
                            if !js_kod.is_empty() {
                                script_node.children.push(DomNode {
                                    node_type: NodeType::Text(js_kod.to_string()),
                                    attributes: Vec::new(),
                                    children: Vec::new(),
                                });
                            }
                            kalan = "";
                        }
                        if let Some(acik_kutu) = stack.last_mut() {
                            acik_kutu.children.push(script_node);
                        }
                    } else {
                        // Normal açılış etiketi (örn: <h1> veya <a href="...">):
                        // Yeni kutuyu aç ve masanın en üstüne koy (içine gelecek yazıları beklesin)
                        stack.push(DomNode::element_with_attrs(&etiket_adi, ozellikler));
                    }
                }
            }
        } else {
            // Hiç '<' kalmadıysa, sona kalan düz yazıyı da açık kutuya ekle ve bitir
            let son_yazi = kalan.trim();
            if !son_yazi.is_empty()
                && let Some(acik_kutu) = stack.last_mut()
            {
                acik_kutu.children.push(DomNode::text(son_yazi));
            }
            break;
        }
    }

    // HATA TOLERANSI (Bozuk HTML Koruması):
    // Eğer siteyi yazan kişi bazı etiketleri kapatmayı unuttuysa,
    // açık kalan tüm kutuları sırayla kapatıp ebeveynlerine bağla!
    while stack.len() > 1 {
        let biten_kutu = stack.pop().unwrap();
        if let Some(ebeveyn_kutu) = stack.last_mut() {
            ebeveyn_kutu.children.push(biten_kutu);
        }
    }

    // En alttaki "document" kutusunu çıkar.
    // Eğer içinde tek bir ana kök etiket (örn: <html> veya <body>) varsa doğrudan onu döndür.
    let mut document = stack.pop().unwrap();
    if document.children.len() == 1 {
        document.children.remove(0)
    } else {
        document
    }
}

/// HTML içindeki özel karakter kodlarını (`&amp;`, `&nbsp;`, `&quot;` vb.)
/// gerçek okunabilir karakterlere dönüştürür.
pub fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

/// DOM ağacındaki `<title>...</title>` etiketinin içindeki sayfa başlığını bulur.
pub fn extract_title(node: &DomNode) -> Option<String> {
    if let NodeType::Element(tag) = &node.node_type
        && tag == "title"
    {
        for child in &node.children {
            if let NodeType::Text(text) = &child.node_type {
                let t = text.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
        return None;
    }

    for child in &node.children {
        if let Some(title) = extract_title(child) {
            return Some(title);
        }
    }
    None
}

/// DOM ağacındaki tüm `<style>...</style>` etiketlerinin içindeki CSS metinlerini toplar.
pub fn extract_styles(node: &DomNode) -> String {
    let mut css = String::new();
    collect_styles(node, &mut css);
    css
}

/// DOM ağacındaki tüm `<script>...</script>` bloklarının içindeki JavaScript kaynak kodlarını sırayla toplar.
pub fn extract_scripts(node: &DomNode) -> Vec<String> {
    let mut scripts = Vec::new();
    collect_scripts(node, &mut scripts);
    scripts
}

fn collect_scripts(node: &DomNode, out: &mut Vec<String>) {
    if let NodeType::Element(tag) = &node.node_type
        && tag == "script"
    {
        for child in &node.children {
            if let NodeType::Text(code) = &child.node_type
                && !code.trim().is_empty()
            {
                out.push(code.clone());
            }
        }
        return;
    }

    for child in &node.children {
        collect_scripts(child, out);
    }
}

/// DOM ağacındaki `<link rel="stylesheet" href="...">` etiketlerinin `href` adreslerini toplar.
pub fn extract_stylesheet_links(node: &DomNode) -> Vec<String> {
    let mut links = Vec::new();
    collect_stylesheet_links(node, &mut links);
    links
}

fn collect_stylesheet_links(node: &DomNode, out: &mut Vec<String>) {
    if let NodeType::Element(tag) = &node.node_type
        && tag == "link"
    {
        let is_stylesheet = node
            .get_attribute("rel")
            .is_some_and(|r| r.eq_ignore_ascii_case("stylesheet"));
        if is_stylesheet && let Some(href) = node.get_attribute("href") {
            let trimmed = href.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
        }
    }

    for child in &node.children {
        collect_stylesheet_links(child, out);
    }
}

fn collect_styles(node: &DomNode, out: &mut String) {
    if let NodeType::Element(tag) = &node.node_type
        && tag == "style"
    {
        for child in &node.children {
            if let NodeType::Text(text) = &child.node_type {
                out.push_str(text);
                out.push('\n');
            }
        }
        return;
    }

    for child in &node.children {
        collect_styles(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_agaci_olusturma() {
        let html = "<body><h1>Merhaba Anka!</h1><p>Yerli tarayici</p></body>";
        let kok = parse_html(html);

        // En dış kutu "body" olmalı ve içinde 2 çocuk (h1 ve p) bulunmalı
        assert_eq!(kok.node_type, NodeType::Element("body".to_string()));
        assert_eq!(kok.children.len(), 2);

        // 1. Çocuk: <h1>Merhaba Anka!</h1>
        let h1 = &kok.children[0];
        assert_eq!(h1.node_type, NodeType::Element("h1".to_string()));
        assert_eq!(h1.children.len(), 1);
        assert_eq!(
            h1.children[0].node_type,
            NodeType::Text("Merhaba Anka!".to_string())
        );

        // 2. Çocuk: <p>Yerli tarayici</p>
        let p = &kok.children[1];
        assert_eq!(p.node_type, NodeType::Element("p".to_string()));
        assert_eq!(p.children.len(), 1);
        assert_eq!(
            p.children[0].node_type,
            NodeType::Text("Yerli tarayici".to_string())
        );
    }

    #[test]
    fn test_kapanmamis_bozuk_html_toleransi() {
        // Kapanış etiketleri (</p> ve </div>) unutulmuş bozuk HTML:
        let bozuk_html = "<div><p>Kapanmayan yazi";
        let kok = parse_html(bozuk_html);

        // Motorumuz çökmemeli, otomatik kapatıp ağacı kurmalı!
        assert_eq!(kok.node_type, NodeType::Element("div".to_string()));
        assert_eq!(kok.children.len(), 1);
        assert_eq!(kok.children[0].node_type, NodeType::Element("p".to_string()));
    }

    #[test]
    fn test_link_href_ozelligi_okuma() {
        let html = "<p><a href=\"Status.html\">Durum Sayfasi</a></p>";
        let kok = parse_html(html);

        let link = &kok.children[0];
        assert_eq!(link.node_type, NodeType::Element("a".to_string()));
        assert_eq!(link.get_attribute("href"), Some("Status.html"));
    }

    #[test]
    fn test_style_etiketi_css_cikarma() {
        let html = "<html><head><style>h1 { color: #ff5733; }</style></head><body><h1>Anka</h1></body></html>";
        let kok = parse_html(html);

        let css = extract_styles(&kok);
        assert!(css.contains("h1 { color: #ff5733; }"));
    }

    #[test]
    fn test_html_entity_ve_title_cikarma() {
        let html = "<html><head><title>Anka &amp; Telek</title></head><body><p>A &lt; B &nbsp; &quot;OK&quot;</p></body></html>";
        let kok = parse_html(html);

        assert_eq!(extract_title(&kok), Some("Anka & Telek".to_string()));
        let p_text = &kok.children[1].children[0].children[0].node_type;
        assert_eq!(*p_text, NodeType::Text("A < B   \"OK\"".to_string()));
    }

    #[test]
    fn test_harici_stylesheet_link_cikarma() {
        let html = "<html><head><link rel=\"stylesheet\" href=\"/css/main.css\"></head><body></body></html>";
        let kok = parse_html(html);

        let links = extract_stylesheet_links(&kok);
        assert_eq!(links, vec!["/css/main.css".to_string()]);
    }

    #[test]
    fn test_script_cikarma_ve_dom_id_guncelleme() {
        let html = "<html><body><p id=\"sayac\">0</p><script>if (a < 5) { a = a + 1; }</script></body></html>";
        let mut kok = parse_html(html);

        // `<script>` içindeki `<` işareti HTML etiketi sanılmadan ham JS olarak çıkarılmalı!
        let scripts = extract_scripts(&kok);
        assert_eq!(scripts, vec!["if (a < 5) { a = a + 1; }".to_string()]);

        // `find_by_id_mut` ile `<p id="sayac">` bulunup canlı olarak metni değiştirilebilmeli!
        let node = kok.find_by_id_mut("sayac").expect("sayac bulunmali");
        assert_eq!(node.get_text(), "0");
        node.set_text("42");
        assert_eq!(node.get_text(), "42");
    }
}
