//! Renders a text buffer to a PNG, as a DOS VGA text screen would look: 10x20 cells, line-drawing
//! characters drawn as lines, other glyphs from a Korean-capable system font.
use ratatui::buffer::Buffer;
use ratatui::style::Color;
use std::collections::HashMap;
use std::path::Path;

pub const CW: usize = 10;
pub const CH: usize = 20;

const FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/nanum/NanumGothicCoding.ttf",
    "/usr/share/fonts/truetype/nanum/NanumGothic.ttf",
    "/usr/share/fonts/truetype/unfonts-core/UnDotum.ttf",
    "/usr/share/fonts/truetype/baekmuk/gulim.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "C:\\Windows\\Fonts\\malgun.ttf",
    "C:\\Windows\\Fonts\\gulim.ttc",
    "/System/Library/Fonts/AppleSDGothicNeo.ttc",
    "/Library/Fonts/AppleGothic.ttf",
];

fn load_font() -> Option<fontdue::Font> {
    let mut list: Vec<String> = std::env::var("MDIR_FONT").into_iter().collect();
    list.extend(FONT_CANDIDATES.iter().map(|s| s.to_string()));
    for p in list {
        if let Ok(d) = std::fs::read(&p) {
            let s = fontdue::FontSettings { collection_index: 0, scale: 40.0, ..Default::default() };
            if let Ok(f) = fontdue::Font::from_bytes(d, s) {
                return Some(f);
            }
        }
    }
    None
}

fn rgb(c: Option<Color>, dflt: (u8, u8, u8)) -> (u8, u8, u8) {
    match c {
        Some(Color::Rgb(r, g, b)) => (r, g, b),
        Some(Color::Indexed(i)) if i < 16 => crate::pal::VGA[i as usize],
        Some(Color::Black) => (0, 0, 0),
        Some(Color::White) | Some(Color::Gray) => (0xaa, 0xaa, 0xaa),
        _ => dflt,
    }
}

/// (up, down, left, right): 0 none, 1 single, 2 double.
fn box_parts(c: char) -> Option<[u8; 4]> {
    Some(match c {
        '─' => [0, 0, 1, 1], '│' => [1, 1, 0, 0], '┌' => [0, 1, 0, 1], '┐' => [0, 1, 1, 0],
        '└' => [1, 0, 0, 1], '┘' => [1, 0, 1, 0], '├' => [1, 1, 0, 1], '┤' => [1, 1, 1, 0],
        '┬' => [0, 1, 1, 1], '┴' => [1, 0, 1, 1], '┼' => [1, 1, 1, 1],
        '═' => [0, 0, 2, 2], '║' => [2, 2, 0, 0], '╔' => [0, 2, 0, 2], '╗' => [0, 2, 2, 0],
        '╚' => [2, 0, 0, 2], '╝' => [2, 0, 2, 0], '╠' => [2, 2, 0, 2], '╣' => [2, 2, 2, 0],
        '╦' => [0, 2, 2, 2], '╩' => [2, 0, 2, 2], '╬' => [2, 2, 2, 2],
        '╒' => [0, 1, 0, 2], '╓' => [0, 2, 0, 1], '╕' => [0, 1, 2, 0], '╖' => [0, 2, 1, 0],
        '╘' => [1, 0, 0, 2], '╙' => [2, 0, 0, 1], '╛' => [1, 0, 2, 0], '╜' => [2, 0, 1, 0],
        '╞' => [1, 1, 0, 2], '╟' => [2, 2, 0, 1], '╡' => [1, 1, 2, 0], '╢' => [2, 2, 1, 0],
        '╤' => [0, 1, 2, 2], '╥' => [0, 2, 1, 1], '╧' => [1, 0, 2, 2], '╨' => [2, 0, 1, 1],
        '╪' => [1, 1, 2, 2], '╫' => [2, 2, 1, 1],
        '━' => [0, 0, 1, 1], '┃' => [1, 1, 0, 0],
        _ => return None,
    })
}

struct Img {
    w: usize,
    px: Vec<u8>,
}

impl Img {
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: (u8, u8, u8)) {
        for yy in y..y + h {
            for xx in x..x + w {
                if xx < self.w && (yy * self.w + xx) * 3 + 2 < self.px.len() {
                    let i = (yy * self.w + xx) * 3;
                    self.px[i] = c.0;
                    self.px[i + 1] = c.1;
                    self.px[i + 2] = c.2;
                }
            }
        }
    }
    fn blend(&mut self, x: usize, y: usize, a: u8, c: (u8, u8, u8)) {
        if x >= self.w {
            return;
        }
        let i = (y * self.w + x) * 3;
        if i + 2 >= self.px.len() {
            return;
        }
        let f = a as u32;
        for (k, v) in [c.0, c.1, c.2].into_iter().enumerate() {
            self.px[i + k] = ((v as u32 * f + self.px[i + k] as u32 * (255 - f)) / 255) as u8;
        }
    }
}

fn draw_box(img: &mut Img, x0: usize, y0: usize, p: [u8; 4], fg: (u8, u8, u8)) {
    let cx = x0 + CW / 2 - 1;
    let cy = y0 + CH / 2 - 1;
    let t = 3;
    let off = |k: u8| -> Vec<isize> { if k == 2 { vec![-3, 3] } else if k == 1 { vec![0] } else { vec![] } };
    let any_double_v = p[0] == 2 || p[1] == 2;
    let any_double_h = p[2] == 2 || p[3] == 2;
    // vertical strokes
    for (part, from, to) in [(p[0], y0, cy + t), (p[1], cy, y0 + CH)] {
        for o in off(part) {
            let ext = if any_double_h { 3 } else { 0 };
            let (a, b) = if from == y0 { (from, to + ext) } else { (from.saturating_sub(ext), to) };
            img.rect((cx as isize + o) as usize, a, t, b - a, fg);
        }
    }
    for (part, from, to) in [(p[2], x0, cx + t), (p[3], cx, x0 + CW)] {
        for o in off(part) {
            let ext = if any_double_v { 3 } else { 0 };
            let (a, b) = if from == x0 { (from, to + ext) } else { (from.saturating_sub(ext), to) };
            img.rect(a, (cy as isize + o) as usize, b - a, t, fg);
        }
    }
}

/// Writes the buffer as a PNG. Returns an error text when no file could be written.
pub fn save_png(buf: &Buffer, path: &Path) -> Result<(), String> {
    let area = buf.area;
    let (w, h) = (area.width as usize * CW, area.height as usize * CH);
    let mut img = Img { w, px: vec![0; w * h * 3] };
    let font = load_font();
    let px = 17.0f32;
    let ascent = font.as_ref().and_then(|f| f.horizontal_line_metrics(px)).map(|m| m.ascent).unwrap_or(14.0);
    let mut cache: HashMap<char, (fontdue::Metrics, Vec<u8>)> = HashMap::new();
    for pass in 0..2 {
    for yy in 0..area.height {
        for xx in 0..area.width {
            let cell = &buf[(area.x + xx, area.y + yy)];
            let fg = rgb(cell.fg.into(), (0xaa, 0xaa, 0xaa));
            let bg = rgb(cell.bg.into(), (0, 0, 0));
            let (x0, y0) = (xx as usize * CW, yy as usize * CH);
            if pass == 0 {
                // the cell after a wide (Hangul) character takes that character's background
                let prev_wide = xx > 0 && crate::text::width(buf[(area.x + xx - 1, area.y + yy)].symbol()) >= 2;
                let bgc = if prev_wide { rgb(buf[(area.x + xx - 1, area.y + yy)].bg.into(), (0, 0, 0)) } else { bg };
                img.rect(x0, y0, CW, CH, bgc);
                continue;
            }
            if xx > 0 && crate::text::width(buf[(area.x + xx - 1, area.y + yy)].symbol()) >= 2 {
                continue;
            }
            let sym = cell.symbol();
            let Some(ch) = sym.chars().next() else { continue };
            if ch == ' ' {
                continue;
            }
            if let Some(p) = box_parts(ch) {
                draw_box(&mut img, x0, y0, p, fg);
                continue;
            }
            match ch {
                '█' => { img.rect(x0, y0, CW, CH, fg); continue; }
                '▀' => { img.rect(x0, y0, CW, CH / 2, fg); continue; }
                '▄' => { img.rect(x0, y0 + CH / 2, CW, CH / 2, fg); continue; }
                '▌' => { img.rect(x0, y0, CW / 2, CH, fg); continue; }
                '▐' => { img.rect(x0 + CW / 2, y0, CW / 2, CH, fg); continue; }
                '░' | '▒' | '▓' => {
                    let a = match ch { '░' => 64, '▒' => 128, _ => 192 };
                    for y in 0..CH { for x in 0..CW { if (x + y) % 2 == 0 || a > 150 { img.blend(x0 + x, y0 + y, a, fg); } } }
                    continue;
                }
                _ => {}
            }
            let Some(f) = font.as_ref() else {
                img.rect(x0 + 2, y0 + 6, CW - 4, CH - 10, fg);
                continue;
            };
            let (m, bm) = cache.entry(ch).or_insert_with(|| f.rasterize(ch, px)).clone();
            let cellw = if crate::text::width(sym) >= 2 { CW * 2 } else { CW };
            let gx = x0 as isize + ((cellw as isize - m.width as isize) / 2).max(0);
            let gy = y0 as isize + 1 + ascent as isize - m.height as isize - m.ymin as isize;
            for (i, a) in bm.iter().enumerate() {
                if *a == 0 {
                    continue;
                }
                let (bx, by) = (gx + (i % m.width.max(1)) as isize, gy + (i / m.width.max(1)) as isize);
                if bx >= 0 && by >= 0 && (by as usize) < h {
                    img.blend(bx as usize, by as usize, *a, fg);
                }
            }
        }
    }
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(&img.px).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod t {
    use super::*;
    use ratatui::layout::Rect;
    #[test]
    fn lines_drawn() {
        let mut b = Buffer::empty(Rect::new(0, 0, 3, 1));
        crate::scr::put(&mut b, 0, 0, "──│", 0x3f);
        let p = std::env::temp_dir().join("mdir-shot-test.png");
        save_png(&b, &p).unwrap();
        let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&p).unwrap()));
        let mut r = dec.read_info().unwrap();
        let mut px = vec![0; r.output_buffer_size()];
        r.next_frame(&mut px).unwrap();
        let at = |x: usize, y: usize| px[(y * 30 + x) * 3];
        assert!(at(2, CH / 2 - 1) > 100, "horizontal line");
        assert!(at(2 * CW + CW / 2, 2) > 100, "vertical line");
    }
}
