//! Screen shared by MF and MCOPY: a title, the command menu on the left and the INFO box on the
//! right, over a patterned background (MF.DOC / MCOPY.DOC menus).
use crate::pal::{Colors, DIALOG, FKEY, FKEY_HI};
use crate::scr::{fill, frame, hi_of, put, put_hl, putw, shadow};
use ratatui::buffer::Buffer;

pub struct Tool<'a> {
    pub title: &'a str,
    pub menu: &'a [String],
    pub sel: usize,
    pub info: &'a [String],
    pub status: &'a str,
    pub progress: Option<&'a str>,
}

pub fn draw(b: &mut Buffer, c: &Colors, t: &Tool) {
    let area = b.area;
    let (w, h) = (area.width, area.height);
    fill(b, 0, 0, w, h, '░', 0x17);
    let fk = c.items[FKEY];
    putw(b, 0, 0, "", w, fk);
    let tw = crate::scr::hw(t.title);
    put_hl(b, (w.saturating_sub(tw)) / 2, 0, t.title, fk, c.items[FKEY_HI]);
    let a = c.items[DIALOG];
    let hi = hi_of(a);
    let mw = t.menu.iter().map(|s| crate::scr::hw(s)).max().unwrap_or(20) + 4;
    let (mx, my) = (4u16, 4u16);
    let mh = t.menu.len() as u16 + 2;
    frame(b, mx, my, mw, mh, a, false);
    shadow(b, mx, my, mw, mh);
    for (i, m) in t.menu.iter().enumerate() {
        let y = my + 1 + i as u16;
        if m.starts_with('-') {
            put(b, mx, y, &format!("├{}┤", "─".repeat(mw as usize - 2)), a);
            continue;
        }
        let (na, ha) = if i == t.sel { (0x70, 0x74) } else { (a, hi) };
        putw(b, mx + 1, y, "", mw - 2, na);
        put_hl(b, mx + 2, y, m, na, ha);
    }
    let iw = t.info.iter().map(|s| crate::scr::hw(s)).max().unwrap_or(20) + 4;
    let ix = (mx + mw + 6).min(w.saturating_sub(iw + 2));
    let ih = t.info.len() as u16 + 4;
    frame(b, ix, my, iw, ih, a, false);
    shadow(b, ix, my, iw, ih);
    put_hl(b, ix + (iw.saturating_sub(10)) / 2, my + 1, "▥▥ ^INFO^ ▥▥", a, hi);
    for (i, l) in t.info.iter().enumerate() {
        if l.starts_with('-') {
            put(b, ix, my + 3 + i as u16, &format!("├{}┤", "─".repeat(iw as usize - 2)), a);
        } else {
            put_hl(b, ix + 2, my + 3 + i as u16, l, a, hi);
        }
    }
    if let Some(p) = t.progress {
        let py = my + ih + 2;
        frame(b, ix, py, iw, 3, a, false);
        put_hl(b, ix + 2, py + 1, p, a, hi);
    }
    putw(b, 0, h - 1, "", w, 0x70);
    put_hl(b, 1, h - 1, t.status, 0x70, 0x74);
}
