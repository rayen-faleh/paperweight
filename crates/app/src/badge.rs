//! The tray icon, drawn at runtime so it can carry the count of overdue and
//! due-today items on every platform (Windows trays cannot show a text title).

const SIZE: u32 = 64;
const ACCENT: [u8; 3] = [0xC4, 0x4D, 0x34];
const INK: [u8; 3] = [0x22, 0x20, 0x1C];

/// 3×5 bitmap digits, row-major, top to bottom.
const DIGITS: [[u8; 5]; 11] = [
    [0b111, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b111],
    [0b000, 0b010, 0b111, 0b010, 0b000], // +
];

struct Canvas(Vec<u8>);

impl Canvas {
    fn new() -> Self {
        Canvas(vec![0; (SIZE * SIZE * 4) as usize])
    }

    fn put(&mut self, x: i32, y: i32, rgb: [u8; 3], a: u8) {
        if x < 0 || y < 0 || x >= SIZE as i32 || y >= SIZE as i32 || a == 0 {
            return;
        }
        let i = ((y as u32 * SIZE + x as u32) * 4) as usize;
        let src = a as f32 / 255.0;
        let dst = self.0[i + 3] as f32 / 255.0;
        let out = src + dst * (1.0 - src);
        for (c, channel) in rgb.iter().enumerate() {
            let v =
                (*channel as f32 * src + self.0[i + c] as f32 * dst * (1.0 - src)) / out.max(1e-6);
            self.0[i + c] = v.round() as u8;
        }
        self.0[i + 3] = (out * 255.0).round() as u8;
    }

    /// Anti-aliased filled circle.
    fn circle(&mut self, cx: f32, cy: f32, r: f32, rgb: [u8; 3]) {
        for y in 0..SIZE as i32 {
            for x in 0..SIZE as i32 {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let cover = (r - d + 0.5).clamp(0.0, 1.0);
                if cover > 0.0 {
                    self.put(x, y, rgb, (cover * 255.0) as u8);
                }
            }
        }
    }

    /// The paperweight: a dome on a flat base.
    fn dome(&mut self, rgb: [u8; 3]) {
        let (cx, base, r) = (32.0f32, 52.0f32, 26.0f32);
        for y in 0..SIZE as i32 {
            for x in 0..SIZE as i32 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                if py > base + 0.5 {
                    continue;
                }
                let d = ((px - cx).powi(2) + (py - base).powi(2)).sqrt();
                let cover = (r - d + 0.5).clamp(0.0, 1.0) * (base + 0.5 - py).clamp(0.0, 1.0);
                if cover > 0.0 {
                    self.put(x, y, rgb, (cover * 255.0) as u8);
                }
            }
        }
        // Base plate.
        for y in 53..58 {
            for x in 4..60 {
                self.put(x, y, rgb, 255);
            }
        }
    }

    fn glyph(&mut self, g: usize, x0: i32, y0: i32, scale: i32, rgb: [u8; 3]) {
        for (row, bits) in DIGITS[g].iter().enumerate() {
            for col in 0..3 {
                if bits & (0b100 >> col) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            self.put(
                                x0 + col * scale + dx,
                                y0 + row as i32 * scale + dy,
                                rgb,
                                255,
                            );
                        }
                    }
                }
            }
        }
    }
}

/// RGBA pixels and size of the tray icon for `count` urgent items.
pub fn render(count: usize) -> (Vec<u8>, u32, u32) {
    let mut c = Canvas::new();
    c.dome(ACCENT);
    if count > 0 {
        let glyphs: Vec<usize> = if count > 9 { vec![9, 10] } else { vec![count] };
        c.circle(44.0, 20.0, 19.0, [0xFF, 0xFF, 0xFF]);
        c.circle(44.0, 20.0, 16.5, INK);
        let scale = if glyphs.len() == 1 { 5 } else { 3 };
        let w = glyphs.len() as i32 * 3 * scale + (glyphs.len() as i32 - 1) * scale;
        let h = 5 * scale;
        let mut x = 44 - w / 2;
        for g in glyphs {
            c.glyph(g, x, 20 - h / 2, scale, [0xFF, 0xFF, 0xFF]);
            x += 4 * scale;
        }
    }
    (c.0, SIZE, SIZE)
}
