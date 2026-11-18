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

    // Bu kutunun içinde bulunan alt kutuların (çocukların) listesi
    pub children: Vec<DomNode>,
}

impl DomNode {
    /// Yeni bir Etiket (Element) kutusu oluşturmak için kısayol fonksiyonu
    pub fn element(tag_name: &str) -> Self {
        DomNode {
            node_type: NodeType::Element(tag_name.to_lowercase()),
            children: Vec::new(),
        }
    }

    /// Yeni bir Düz Yazı (Text) kutusu oluşturmak için kısayol fonksiyonu
    pub fn text(content: &str) -> Self {
        DomNode {
            node_type: NodeType::Text(content.to_string()),
            children: Vec::new(),
        }
    }
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

                // Etiketin içinde boşluk/özellik varsa sadece ilk kelimeyi (etiket adını) al
                let etiket_adi = temiz_etiket
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_lowercase();

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
                        acik_kutu.children.push(DomNode::element(&etiket_adi));
                    }
                } else {
                    // Normal açılış etiketi (örn: <h1>):
                    // Yeni kutuyu aç ve masanın en üstüne koy (içine gelecek yazıları beklesin)
                    stack.push(DomNode::element(&etiket_adi));
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
}
