// Anka Browser - Çizim Listesi (Display List), RGB Renk Motoru ve Görsel Çözücü (İstasyon 6)
// Sıfır harici crate bağımlılığıyla BMP ve PPM (P6) görsellerini saf Rust ile,
// Linux üzerinde ise PNG / JPEG / GIF / WebP dosyalarını sistemdeki `gdk-pixbuf` ile çözer!

use anka_layout::LayoutBox;

/// Bilgisayar ekranındaki bir pikselin Kırmızı (R), Yeşil (G) ve Mavi (B) renk değerleri (0 - 255)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    /// CSS renk adını (örn: "red") veya Hex kodunu (örn: "#ff5733")
    /// bilgisayarın anlayacağı (R, G, B) piksel koduna çevirir.
    pub fn from_css_name(name: &str) -> Self {
        let temiz = name.trim().to_lowercase();

        // 1. DURUM: Renk '#' ile başlıyorsa Hex (16'lık taban) renk kodudur!
        // Örn: "#ff5733" -> "ff" (255), "57" (87), "33" (51)
        if let Some(hex) = temiz.strip_prefix('#') {
            if hex.len() == 6
                && let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..2], 16),
                    u8::from_str_radix(&hex[2..4], 16),
                    u8::from_str_radix(&hex[4..6], 16),
                )
            {
                return RgbColor { r, g, b };
            }
            // Kısa 3 haneli Hex desteği (Örn: "#f00" -> "ff", "00", "00")
            if hex.len() == 3
                && let (Ok(r), Ok(g), Ok(b)) = (
                    u8::from_str_radix(&hex[0..1], 16),
                    u8::from_str_radix(&hex[1..2], 16),
                    u8::from_str_radix(&hex[2..3], 16),
                )
            {
                return RgbColor {
                    r: r * 17,
                    g: g * 17,
                    b: b * 17,
                };
            }
        }

        // 2. DURUM: Standart İngilizce renk kelimeleri
        match temiz.as_str() {
            "red" => RgbColor { r: 220, g: 38, b: 38 },
            "blue" => RgbColor { r: 37, g: 99, b: 235 },
            "green" => RgbColor { r: 22, g: 163, b: 74 },
            "yellow" => RgbColor { r: 234, g: 179, b: 8 },
            "white" => RgbColor {
                r: 255,
                g: 255,
                b: 255,
            },
            "gray" | "grey" => RgbColor {
                r: 107,
                g: 114,
                b: 128,
            },
            // Varsayılan renk: Siyah
            _ => RgbColor { r: 0, g: 0, b: 0 },
        }
    }
}

/// Ekrana doğrudan çizilebilecek şekilde ham piksellerine (`R, G, B, A`) çözülmüş görsel paketi:
/// - `width`: Görselin piksel cinsinden genişliği
/// - `height`: Görselin piksel cinsinden yüksekliği
/// - `rgba`: Soldan sağa, yukarıdan aşağıya her piksel için 4 bayt (`[R, G, B, A]`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl DecodedImage {
    /// Verilen ham resim baytlarını (BMP, PPM P6 veya Linux'ta PNG/JPEG/GIF/WebP)
    /// otomatik tanıyarak piksellere (`DecodedImage`) dönüştürür.
    pub fn decode_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        // 1. Önce kendi yazdığımız saf Rust BMP çözücüsünü dene (`"BM"` imzası)
        if bytes.starts_with(b"BM") {
            return Self::decode_bmp(bytes);
        }

        // 2. Saf Rust PPM (P6 ikili RGB) çözücüsünü dene (`"P6"` imzası)
        if bytes.starts_with(b"P6") {
            return Self::decode_ppm_p6(bytes);
        }

        // 3. Linux üzerinde PNG, JPEG, GIF, WebP gibi sıkıştırılmış formatları
        // sistemin yerleşik `libgdk_pixbuf-2.0.so` kütüphanesiyle çöz!
        #[cfg(target_os = "linux")]
        {
            linux_pixbuf::decode_with_gdk_pixbuf(bytes)
        }

        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }

    /// SIFIR BAĞIMLILIKLI SAF RUST BMP ÇÖZÜCÜSÜ (24-bit ve 32-bit sıkıştırılmamış BMP):
    /// Bir BMP dosyasının bayt yapısını adım adım okuyarak RGBA piksellerine çevirir:
    /// - 0..2: `"BM"` dosya imzası
    /// - 10..14: Piksel verisinin başladığı bayt konumu (`pixel_offset`)
    /// - 18..22: Genişlik (`width`), 22..26: Yükseklik (`height`, pozitifse alttan üste sıralıdır!)
    /// - 28..30: Piksel başına bit sayısı (`24` = BGR, `32` = BGRA)
    pub fn decode_bmp(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 54 || &bytes[0..2] != b"BM" {
            return None;
        }

        let pixel_offset = u32::from_le_bytes(bytes[10..14].try_into().ok()?) as usize;
        let raw_w = i32::from_le_bytes(bytes[18..22].try_into().ok()?);
        let raw_h = i32::from_le_bytes(bytes[22..26].try_into().ok()?);
        let bpp = u16::from_le_bytes(bytes[28..30].try_into().ok()?);
        let compression = u32::from_le_bytes(bytes[30..34].try_into().ok()?);

        // Yalnızca sıkıştırılmamış (BI_RGB = 0) 24-bit veya 32-bit BMP desteklenir
        if raw_w <= 0 || raw_h == 0 || compression != 0 || (bpp != 24 && bpp != 32) {
            return None;
        }

        let width = raw_w as u32;
        let height = raw_h.unsigned_abs();
        let bottom_up = raw_h > 0;

        // Aşırı büyük/bozuk başlıklara karşı bellek taşması koruması
        if width > 4096 || height > 4096 {
            return None;
        }

        let bytes_per_pixel = (bpp / 8) as usize;
        // BMP formatında her piksel satırı 4 baytın katı olacak şekilde hizalanır (row padding)
        let row_stride = (width as usize * bytes_per_pixel).div_ceil(4) * 4;
        let needed = pixel_offset.checked_add(row_stride.checked_mul(height as usize)?)?;
        if bytes.len() < needed {
            return None;
        }

        let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];

        for y in 0..height as usize {
            let src_y = if bottom_up {
                (height as usize - 1) - y
            } else {
                y
            };
            let row_start = pixel_offset + src_y * row_stride;

            for x in 0..width as usize {
                let src_idx = row_start + x * bytes_per_pixel;
                let dst_idx = (y * width as usize + x) * 4;
                // BMP pikselleri hafızada BGR(A) sırasındadır -> Biz RGBA'ya çeviriyoruz!
                rgba[dst_idx] = bytes[src_idx + 2]; // R
                rgba[dst_idx + 1] = bytes[src_idx + 1]; // G
                rgba[dst_idx + 2] = bytes[src_idx]; // B
                rgba[dst_idx + 3] = if bytes_per_pixel == 4 {
                    bytes[src_idx + 3]
                } else {
                    255
                };
            }
        }

        Some(DecodedImage {
            width,
            height,
            rgba,
        })
    }

    /// SIFIR BAĞIMLILIKLI SAF RUST PPM (`P6`) ÇÖZÜCÜSÜ:
    /// Başlık formatı: `P6\n<genislik> <yukseklik>\n255\n<R,G,B baytlari...>`
    pub fn decode_ppm_p6(bytes: &[u8]) -> Option<Self> {
        let mut idx = 0;
        let mut tokens = Vec::with_capacity(4);

        while tokens.len() < 4 && idx < bytes.len() {
            // Boşlukları atla
            while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
                idx += 1;
            }
            // `#` ile başlayan PPM yorum satırlarını atla
            if idx < bytes.len() && bytes[idx] == b'#' {
                while idx < bytes.len() && bytes[idx] != b'\n' {
                    idx += 1;
                }
                continue;
            }
            let start = idx;
            while idx < bytes.len() && !bytes[idx].is_ascii_whitespace() {
                idx += 1;
            }
            if idx > start {
                tokens.push(std::str::from_utf8(&bytes[start..idx]).ok()?);
            }
        }

        if tokens.len() < 4 || tokens[0] != "P6" {
            return None;
        }
        let width: u32 = tokens[1].parse().ok()?;
        let height: u32 = tokens[2].parse().ok()?;
        let max_val: u32 = tokens[3].parse().ok()?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 || max_val != 255 {
            return None;
        }

        // Başlıktan sonra tam 1 boşluk karakteri (genellikle `\n`) bulunur
        if idx >= bytes.len() {
            return None;
        }
        let pixel_start = idx + 1;
        let expected_rgb = (width as usize) * (height as usize) * 3;
        if bytes.len() < pixel_start + expected_rgb {
            return None;
        }

        let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);
        let (rgb_chunks, _) = bytes[pixel_start..pixel_start + expected_rgb].as_chunks::<3>();
        for &[r, g, b] in rgb_chunks {
            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(255);
        }

        Some(DecodedImage {
            width,
            height,
            rgba,
        })
    }

    /// Telek Engine'in yerleşik (çevrimdışı test amaçlı) **Anka Kuşu / Gün Batımı Amblemi** görselini
    /// saf piksel matematiğiyle üretir (`http://anka.local/anka-logo.bmp` için kullanılır).
    pub fn generate_anka_banner(width: u32, height: u32) -> Self {
        let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
        let w_f = width.max(1) as f32;
        let h_f = height.max(1) as f32;

        for y in 0..height {
            for x in 0..width {
                let u = x as f32 / w_f;
                let v = y as f32 / h_f;

                // 1. Arka plan: Gece lacivertinden kızıl gün batımına yumuşak geçiş (Gradient)
                let mut r = (24.0 + u * 165.0 + v * 35.0).min(255.0) as u8;
                let mut g = (34.0 + (1.0 - u) * 35.0 + v * 45.0).min(255.0) as u8;
                let mut b = (68.0 + (1.0 - u) * 90.0).min(255.0) as u8;

                // 2. Ortada parlayan güneş diski (Anka ateşi)
                let dx = (x as f32 - w_f * 0.28) / (h_f * 0.32);
                let dy = (y as f32 - h_f * 0.50) / (h_f * 0.32);
                let dist = (dx * dx + dy * dy).sqrt();
                if dist < 1.0 {
                    // Altın sarısı -> ateş turuncusu çekirdek
                    r = 250;
                    g = (210.0 - dist * 110.0).max(80.0) as u8;
                    b = (70.0 - dist * 50.0).max(15.0) as u8;
                }

                // 3. Stilize Anka kanatları (V şeklinde simetrik iki kanat çizgisi)
                let wx = (x as f32 - w_f * 0.28) / (h_f * 0.45);
                let wy = (y as f32 - h_f * 0.50) / (h_f * 0.45);
                let wing_curve = (wx.abs() * 0.65) - 0.15;
                if wx.abs() < 0.85 && (wy + wing_curve).abs() < 0.10 {
                    r = 255;
                    g = 248;
                    b = 220;
                }

                // 4. Sağ tarafta Telek Engine renk spektrumu çubukları (RGB piksel doğrulaması!)
                if u > 0.56 && u < 0.92 && v > 0.24 && v < 0.76 {
                    let stripe = ((v - 0.24) / 0.13) as u32;
                    match stripe {
                        0 => {
                            r = 239;
                            g = 68;
                            b = 68;
                        } // Kızıl
                        1 => {
                            r = 245;
                            g = 158;
                            b = 11;
                        } // Kehribar
                        2 => {
                            r = 16;
                            g = 185;
                            b = 129;
                        } // Zümrüt
                        _ => {
                            r = 59;
                            g = 130;
                            b = 246;
                        } // Gökyüzü
                    }
                }

                // 5. İnce dış çerçeve
                if x < 2 || y < 2 || x + 2 >= width || y + 2 >= height {
                    r = 248;
                    g = 250;
                    b = 252;
                }

                let idx = (y as usize * width as usize + x as usize) * 4;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }

        DecodedImage {
            width,
            height,
            rgba,
        }
    }
}

/// Linux ortamında PNG, JPEG, GIF ve WebP görsellerini sistemde kurulu `libgdk_pixbuf-2.0.so`
/// kütüphanesi üzerinden sıfır harici crate ile RGBA piksellerine çözen alt modül:
#[cfg(target_os = "linux")]
mod linux_pixbuf {
    use std::ffi::{c_int, c_void};
    use std::ptr::null_mut;

    use super::DecodedImage;

    type GdkPixbufLoader = c_void;
    type GdkPixbuf = c_void;

    #[link(name = "gdk_pixbuf-2.0")]
    unsafe extern "C" {
        fn gdk_pixbuf_loader_new() -> *mut GdkPixbufLoader;
        fn gdk_pixbuf_loader_write(
            loader: *mut GdkPixbufLoader,
            buf: *const u8,
            count: usize,
            error: *mut *mut c_void,
        ) -> c_int;
        fn gdk_pixbuf_loader_close(loader: *mut GdkPixbufLoader, error: *mut *mut c_void) -> c_int;
        fn gdk_pixbuf_loader_get_pixbuf(loader: *mut GdkPixbufLoader) -> *mut GdkPixbuf;
        fn gdk_pixbuf_get_width(pixbuf: *const GdkPixbuf) -> c_int;
        fn gdk_pixbuf_get_height(pixbuf: *const GdkPixbuf) -> c_int;
        fn gdk_pixbuf_get_rowstride(pixbuf: *const GdkPixbuf) -> c_int;
        fn gdk_pixbuf_get_n_channels(pixbuf: *const GdkPixbuf) -> c_int;
        fn gdk_pixbuf_get_pixels(pixbuf: *const GdkPixbuf) -> *const u8;
    }

    #[link(name = "gobject-2.0")]
    unsafe extern "C" {
        fn g_object_unref(object: *mut c_void);
    }

    pub fn decode_with_gdk_pixbuf(bytes: &[u8]) -> Option<DecodedImage> {
        unsafe {
            let loader = gdk_pixbuf_loader_new();
            if loader.is_null() {
                return None;
            }

            let write_ok = gdk_pixbuf_loader_write(loader, bytes.as_ptr(), bytes.len(), null_mut());
            let close_ok = gdk_pixbuf_loader_close(loader, null_mut());
            if write_ok == 0 || close_ok == 0 {
                g_object_unref(loader);
                return None;
            }

            let pixbuf = gdk_pixbuf_loader_get_pixbuf(loader);
            if pixbuf.is_null() {
                g_object_unref(loader);
                return None;
            }

            let w = gdk_pixbuf_get_width(pixbuf);
            let h = gdk_pixbuf_get_height(pixbuf);
            let rowstride = gdk_pixbuf_get_rowstride(pixbuf);
            let channels = gdk_pixbuf_get_n_channels(pixbuf);
            let pixels_ptr = gdk_pixbuf_get_pixels(pixbuf);

            if w <= 0
                || h <= 0
                || w > 4096
                || h > 4096
                || (channels != 3 && channels != 4)
                || pixels_ptr.is_null()
            {
                g_object_unref(loader);
                return None;
            }

            let width = w as u32;
            let height = h as u32;
            let stride = rowstride as usize;
            let ch = channels as usize;
            let total_src = stride * (height as usize);
            let src = std::slice::from_raw_parts(pixels_ptr, total_src);

            let mut rgba = vec![0u8; (width as usize) * (height as usize) * 4];
            for y in 0..height as usize {
                let row = &src[y * stride..];
                for x in 0..width as usize {
                    let s = x * ch;
                    let d = (y * width as usize + x) * 4;
                    rgba[d] = row[s];
                    rgba[d + 1] = row[s + 1];
                    rgba[d + 2] = row[s + 2];
                    rgba[d + 3] = if ch == 4 { row[s + 3] } else { 255 };
                }
            }

            // `pixbuf` sahipliği `loader`'a aittir; `loader`'ı serbest bırakınca ikisi de temizlenir.
            g_object_unref(loader);

            Some(DecodedImage {
                width,
                height,
                rgba,
            })
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
    pub bg_color: Option<RgbColor>,
    pub border_color: Option<RgbColor>,
    pub href: Option<String>,
    /// Eğer bu komut bir `<img>` etiketi ise kaynak adresi (`src`)
    pub image_src: Option<String>,
    /// İndirilip piksellerine çözülmüş gerçek görsel verisi (varsa)
    pub image: Option<DecodedImage>,
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
            bg_color: b.bg_color.as_deref().map(RgbColor::from_css_name),
            border_color: b.border_color.as_deref().map(RgbColor::from_css_name),
            href: b.href.clone(),
            image_src: b.image_src.clone(),
            image: None,
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
            bg_color: Some("#f1f5f9".to_string()),
            border_color: Some("#cbd5e1".to_string()),
            font_size: 40,
            href: None,
            image_src: None,
        }];

        let cizim_listesi = build_display_list(&kutular);

        assert_eq!(cizim_listesi.len(), 1);
        assert_eq!(cizim_listesi[0].text, "Anka Browser");
        assert_eq!(
            cizim_listesi[0].color,
            RgbColor {
                r: 220,
                g: 38,
                b: 38
            }
        );
        assert_eq!(
            cizim_listesi[0].bg_color,
            Some(RgbColor {
                r: 241,
                g: 245,
                b: 249
            })
        );
    }

    #[test]
    fn test_hex_renk_cozumleme() {
        // 6 haneli Hex kodu (#ff5733 -> R:255, G:87, B:51)
        assert_eq!(
            RgbColor::from_css_name("#ff5733"),
            RgbColor {
                r: 255,
                g: 87,
                b: 51
            }
        );

        // 3 haneli kısa Hex kodu (#0f0 -> R:0, G:255, B:0)
        assert_eq!(
            RgbColor::from_css_name("#0f0"),
            RgbColor { r: 0, g: 255, b: 0 }
        );
    }

    #[test]
    fn test_saf_rust_bmp_gorsel_cozucu() {
        // 2x1 piksellik 24-bit sıkıştırılmamış geçerli bir BMP dosyası üret:
        // Sol piksel: Kırmızı (BGR: 0, 0, 255), Sağ piksel: Mavi (BGR: 255, 0, 0)
        // 2 piksel * 3 bayt = 6 bayt -> 4'ün katı olması için 2 bayt dolgu (padding) = 8 bayt
        let mut bmp = vec![0u8; 54 + 8];
        bmp[0] = b'B';
        bmp[1] = b'M';
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes()); // pixel_offset
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes()); // DIB header size
        bmp[18..22].copy_from_slice(&2i32.to_le_bytes()); // width = 2
        bmp[22..26].copy_from_slice(&1i32.to_le_bytes()); // height = 1
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes()); // planes = 1
        bmp[28..30].copy_from_slice(&24u16.to_le_bytes()); // 24-bit BGR
        // Piksel 0 (Kırmızı): B=0, G=0, R=255
        bmp[54] = 0;
        bmp[55] = 0;
        bmp[56] = 255;
        // Piksel 1 (Mavi): B=255, G=0, R=0
        bmp[57] = 255;
        bmp[58] = 0;
        bmp[59] = 0;

        let img = DecodedImage::decode_bytes(&bmp).expect("BMP cozulebilmeli");
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(&img.rgba[0..4], &[255, 0, 0, 255]); // Sol piksel: Kırmızı RGBA
        assert_eq!(&img.rgba[4..8], &[0, 0, 255, 255]); // Sağ piksel: Mavi RGBA
    }

    #[test]
    fn test_saf_rust_ppm_p6_gorsel_cozucu() {
        let mut ppm = b"P6\n2 1\n255\n".to_vec();
        ppm.extend_from_slice(&[10, 200, 30, 250, 120, 5]);

        let img = DecodedImage::decode_bytes(&ppm).expect("PPM P6 cozulebilmeli");
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 1);
        assert_eq!(&img.rgba[0..4], &[10, 200, 30, 255]);
        assert_eq!(&img.rgba[4..8], &[250, 120, 5, 255]);
    }
}
