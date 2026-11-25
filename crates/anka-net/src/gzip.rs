// Anka Browser - Sıfır Bağımlılıklı Saf Rust GZIP (RFC 1952) & DEFLATE (RFC 1951) Açıcısı
// Hem Windows'ta hem tüm Linux dağıtımlarında sıfır harici crate ve sıfır C kütüphanesiyle
// `Content-Encoding: gzip` veya `deflate` ile sıkıştırılmış web sayfalarını çözer!

/// Bit akışını en düşük anlamlı bitten (LSB-first) en yükseğe doğru okuyan yardımcı okuyucu:
struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_buf: u32,
    bit_count: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_buf: 0,
            bit_count: 0,
        }
    }

    fn ensure_bits(&mut self, n: u8) -> bool {
        while self.bit_count < n {
            if self.byte_pos >= self.data.len() {
                return false;
            }
            self.bit_buf |= (self.data[self.byte_pos] as u32) << self.bit_count;
            self.byte_pos += 1;
            self.bit_count += 8;
        }
        true
    }

    fn read_bits(&mut self, n: u8) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        if !self.ensure_bits(n) {
            return None;
        }
        let val = self.bit_buf & ((1u32 << n) - 1);
        self.bit_buf >>= n;
        self.bit_count -= n;
        Some(val)
    }

    /// Sıkıştırılmamış (stored) bloklarda bir sonraki tam bayt sınırına hizalanır.
    fn align_to_byte(&mut self) {
        let drop_bits = self.bit_count % 8;
        self.bit_buf >>= drop_bits;
        self.bit_count -= drop_bits;
    }

    fn read_u16_le_aligned(&mut self) -> Option<u16> {
        let lo = self.read_bits(8)? as u16;
        let hi = self.read_bits(8)? as u16;
        Some(lo | (hi << 8))
    }
}

/// Kanonik Huffman kod tablosu (`lengths` dizisinden RFC 1951 kurallarına göre üretilir):
struct HuffmanTable {
    max_bits: u8,
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl HuffmanTable {
    fn from_lengths(lengths: &[u8]) -> Option<Self> {
        let mut counts = [0u16; 16];
        let mut max_bits = 0u8;
        for &len in lengths {
            if len > 15 {
                return None;
            }
            if len > 0 {
                counts[len as usize] += 1;
                if len > max_bits {
                    max_bits = len;
                }
            }
        }

        let mut offsets = [0u16; 16];
        let mut total = 0u16;
        for bits in 1..=15 {
            offsets[bits] = total;
            total = total.checked_add(counts[bits])?;
        }

        let mut symbols = vec![0u16; total as usize];
        for (sym, &len) in lengths.iter().enumerate() {
            if len > 0 {
                let idx = offsets[len as usize] as usize;
                symbols[idx] = sym as u16;
                offsets[len as usize] += 1;
            }
        }

        Some(Self {
            max_bits,
            counts,
            symbols,
        })
    }

    /// Bit akışından tek bir Huffman sembolü okur.
    fn decode(&self, reader: &mut BitReader<'_>) -> Option<u16> {
        let mut code: i32 = 0;
        let mut first: i32 = 0;
        let mut index: i32 = 0;

        for len in 1..=self.max_bits as usize {
            code |= reader.read_bits(1)? as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return self.symbols.get((index + (code - first)) as usize).copied();
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

// RFC 1951 sabit uzunluk (Length) ve mesafe (Distance) tabloları:
const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const CODE_LEN_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// GZIP (`RFC 1952`) başlığını doğrulayıp içindeki DEFLATE sıkıştırılmış veriyi açar.
pub fn decompress_gzip(input: &[u8]) -> Option<Vec<u8>> {
    // 1. ADIM: GZIP sihirli baytları (`0x1f`, `0x8b`) ve sıkıştırma yöntemi (`8 = deflate`)
    if input.len() < 18 || input[0] != 0x1f || input[1] != 0x8b || input[2] != 8 {
        return None;
    }

    let flg = input[3];
    let mut pos = 10usize;

    // FEXTRA alanı varsa atla
    if (flg & 0x04) != 0 {
        if pos + 2 > input.len() {
            return None;
        }
        let xlen = u16::from_le_bytes([input[pos], input[pos + 1]]) as usize;
        pos += 2 + xlen;
    }
    // FNAME (orijinal dosya adı, \0 bitimli) varsa atla
    if (flg & 0x08) != 0 {
        while pos < input.len() && input[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }
    // FCOMMENT (\0 bitimli yorum) varsa atla
    if (flg & 0x10) != 0 {
        while pos < input.len() && input[pos] != 0 {
            pos += 1;
        }
        pos += 1;
    }
    // FHCRC (2 baytlık başlık sağlama toplamı) varsa atla
    if (flg & 0x02) != 0 {
        pos += 2;
    }

    // Sondaki 8 bayt (CRC32 + ISIZE) hariç kalan kısım ham DEFLATE akışıdır:
    if pos + 8 > input.len() {
        return None;
    }

    inflate_raw(&input[pos..input.len() - 8])
}

/// Ham DEFLATE (`RFC 1951`) bayt akışını (Stored + Fixed Huffman + Dynamic Huffman) açar.
pub fn inflate_raw(input: &[u8]) -> Option<Vec<u8>> {
    let mut reader = BitReader::new(input);
    let mut out = Vec::new();
    // Aşırı büyük zip-bomb saldırılarına karşı 16 MB güvenlik sınırı:
    const MAX_OUTPUT: usize = 16 * 1024 * 1024;

    loop {
        let bfinal = reader.read_bits(1)?;
        let btype = reader.read_bits(2)?;

        match btype {
            // 0: Sıkıştırılmamış ham blok (Stored)
            0 => {
                reader.align_to_byte();
                let len = reader.read_u16_le_aligned()? as usize;
                let nlen = reader.read_u16_le_aligned()?;
                if (len as u16) != !nlen {
                    return None;
                }
                for _ in 0..len {
                    out.push(reader.read_bits(8)? as u8);
                    if out.len() > MAX_OUTPUT {
                        return None;
                    }
                }
            }
            // 1: Sabit (Fixed) Huffman tablosu
            1 => {
                let mut lit_lens = [0u8; 288];
                lit_lens[..=143].fill(8);
                lit_lens[144..=255].fill(9);
                lit_lens[256..=279].fill(7);
                lit_lens[280..=287].fill(8);
                let dist_lens = [5u8; 32];

                let lit_table = HuffmanTable::from_lengths(&lit_lens)?;
                let dist_table = HuffmanTable::from_lengths(&dist_lens)?;
                decode_huffman_block(&mut reader, &lit_table, &dist_table, &mut out, MAX_OUTPUT)?;
            }
            // 2: Dinamik (Dynamic) Huffman tablosu (Gerçek web sunucularının %99'u bunu kullanır!)
            2 => {
                let hlit = reader.read_bits(5)? as usize + 257;
                let hdist = reader.read_bits(5)? as usize + 1;
                let hclen = reader.read_bits(4)? as usize + 4;

                let mut cl_lens = [0u8; 19];
                for i in 0..hclen {
                    cl_lens[CODE_LEN_ORDER[i]] = reader.read_bits(3)? as u8;
                }
                let cl_table = HuffmanTable::from_lengths(&cl_lens)?;

                let total_codes = hlit + hdist;
                let mut all_lens = Vec::with_capacity(total_codes);
                while all_lens.len() < total_codes {
                    let sym = cl_table.decode(&mut reader)?;
                    match sym {
                        0..=15 => all_lens.push(sym as u8),
                        16 => {
                            let prev = *all_lens.last()?;
                            let repeat = reader.read_bits(2)? as usize + 3;
                            all_lens.extend(std::iter::repeat_n(prev, repeat));
                        }
                        17 => {
                            let repeat = reader.read_bits(3)? as usize + 3;
                            all_lens.extend(std::iter::repeat_n(0, repeat));
                        }
                        18 => {
                            let repeat = reader.read_bits(7)? as usize + 11;
                            all_lens.extend(std::iter::repeat_n(0, repeat));
                        }
                        _ => return None,
                    }
                }
                if all_lens.len() != total_codes {
                    return None;
                }

                let lit_table = HuffmanTable::from_lengths(&all_lens[..hlit])?;
                let dist_table = HuffmanTable::from_lengths(&all_lens[hlit..])?;
                decode_huffman_block(&mut reader, &lit_table, &dist_table, &mut out, MAX_OUTPUT)?;
            }
            _ => return None,
        }

        if bfinal == 1 {
            break;
        }
    }

    Some(out)
}

fn decode_huffman_block(
    reader: &mut BitReader<'_>,
    lit_table: &HuffmanTable,
    dist_table: &HuffmanTable,
    out: &mut Vec<u8>,
    max_output: usize,
) -> Option<()> {
    loop {
        let sym = lit_table.decode(reader)?;
        match sym {
            0..=255 => {
                out.push(sym as u8);
                if out.len() > max_output {
                    return None;
                }
            }
            256 => break, // Blok sonu (End of Block)
            257..=285 => {
                let len_idx = (sym - 257) as usize;
                let length =
                    LEN_BASE[len_idx] as usize + reader.read_bits(LEN_EXTRA[len_idx])? as usize;

                let dist_sym = dist_table.decode(reader)? as usize;
                if dist_sym >= DIST_BASE.len() {
                    return None;
                }
                let distance = DIST_BASE[dist_sym] as usize
                    + reader.read_bits(DIST_EXTRA[dist_sym])? as usize;

                if distance == 0 || distance > out.len() {
                    return None;
                }
                if out.len() + length > max_output {
                    return None;
                }

                // LZ77 geriye dönük kopyalama (üst üste binen mesafelerde bayt bayt kopyalar!)
                for _ in 0..length {
                    let b = out[out.len() - distance];
                    out.push(b);
                }
            }
            _ => return None,
        }
    }
    Some(())
}
