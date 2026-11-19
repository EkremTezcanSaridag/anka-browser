// Anka Browser - Çizim Listesi (Display List) ve RGB Renk Motoru (İstasyon 6A)

use anka_layout::LayoutBox;

/// Bilgisayar ekranındaki bir pikselin Kırmızı (R), Yeşil (G) ve Mavi (B) renk değerleri (0 - 255)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    /// CSS renk adını (örn: "red", "blue") bilgisayarın anlayacağı (R, G, B) piksel koduna çevirir
    pub fn from_css_name(name: &str) -> Self {
        match name.trim().to_lowercase().as_str() {
            "red" => RgbColor { r: 220, g: 38, b: 38 },
            "blue" => RgbColor { r: 37, g: 99, b: 235 },
            "green" => RgbColor { r: 22, g: 163, b: 74 },
            "yellow" => RgbColor { r: 234, g: 179, b: 8 },
            "white" => RgbColor { r: 255, g: 255, b: 255 },
            "gray" | "grey" => RgbColor { r: 107, g: 114, b: 128 },
            // Varsayılan renk: Siyah
            _ => RgbColor { r: 0, g: 0, b: 0 },
        }
    }
}

/// Ekran kartına / pencereye gönderilecek tek bir çizim emri:
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintCommand {
    pub tag: String,
    pub text: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub font_size: u32,
    pub color: RgbColor,
    pub href: Option<String>,
}

/// `anka-layout` modülünden gelen koordinatlı kutuları (`LayoutBox`),
/// ekranın doğrudan boyayabileceği RGB renkli çizim emirlerine (`PaintCommand`) dönüştürür.
pub fn build_display_list(boxes: &[LayoutBox]) -> Vec<PaintCommand> {
    let mut commands = Vec::new();

    for b in boxes {
        commands.push(PaintCommand {
            tag: b.tag.clone(),
            text: b.text.clone(),
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
            font_size: b.font_size,
            color: RgbColor::from_css_name(&b.color),
            href: b.href.clone(),
        });
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renk_donusumu_ve_cizim_listesi() {
        let kutular = vec![LayoutBox {
            tag: "h1".to_string(),
            text: "Anka Browser".to_string(),
            x: 16,
            y: 16,
            width: 768,
            height: 48,
            color: "red".to_string(),
            font_size: 40,
            href: None,
        }];

        let cizim_listesi = build_display_list(&kutular);

        assert_eq!(cizim_listesi.len(), 1);
        assert_eq!(cizim_listesi[0].text, "Anka Browser");
        assert_eq!(
            cizim_listesi[0].color,
            RgbColor { r: 220, g: 38, b: 38 }
        );
    }
}
