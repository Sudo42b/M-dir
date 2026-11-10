//! Line input (with MdirIII's "fresh initial value" behaviour), message boxes, Yes/No-style
//! choice boxes and list menus.
use crate::pal::{Colors, DIALOG};
use crate::scr::{center_box, hi_of, hw, put, put_hl, putw};
use crate::text::{fit, t, width};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn ctrl(k: &KeyEvent) -> bool {
    k.modifiers.contains(KeyModifiers::CONTROL)
}
pub fn alt(k: &KeyEvent) -> bool {
    k.modifiers.contains(KeyModifiers::ALT)
}
pub fn shift(k: &KeyEvent) -> bool {
    k.modifiers.contains(KeyModifiers::SHIFT)
}

#[derive(Clone, Debug)]
pub struct LineEdit {
    pub text: Vec<char>,
    pub cur: usize,
    /// The initial value is shown highlighted: typing replaces it, arrows/Home/Space start editing it.
    pub fresh: bool,
    pub max: usize,
}

pub enum Edit {
    Done,
    Cancel,
    Changed,
    Unhandled,
}

impl LineEdit {
    pub fn new(init: &str) -> Self {
        let text: Vec<char> = init.chars().collect();
        LineEdit { cur: text.len(), text, fresh: !init.is_empty(), max: 255 }
    }
    pub fn value(&self) -> String {
        self.text.iter().collect()
    }
    pub fn set(&mut self, s: &str) {
        self.text = s.chars().collect();
        self.cur = self.text.len();
    }
    pub fn insert_str(&mut self, s: &str) {
        if self.fresh {
            self.fresh = false;
        }
        for c in s.chars() {
            if self.text.len() < self.max {
                self.text.insert(self.cur, c);
                self.cur += 1;
            }
        }
    }
    pub fn key(&mut self, k: &KeyEvent) -> Edit {
        match k.code {
            KeyCode::Enter if !ctrl(k) && !alt(k) => return Edit::Done,
            KeyCode::Esc => return Edit::Cancel,
            KeyCode::Char('y') if ctrl(k) => {
                self.text.clear();
                self.cur = 0;
                self.fresh = false;
            }
            KeyCode::Char(' ') if self.fresh => self.fresh = false,
            KeyCode::Char(c) if !ctrl(k) && !alt(k) => {
                if self.fresh {
                    self.text.clear();
                    self.cur = 0;
                    self.fresh = false;
                }
                if self.text.len() < self.max {
                    self.text.insert(self.cur, c);
                    self.cur += 1;
                }
            }
            KeyCode::Left => {
                self.fresh = false;
                self.cur = self.cur.saturating_sub(1)
            }
            KeyCode::Right => {
                self.fresh = false;
                self.cur = (self.cur + 1).min(self.text.len())
            }
            KeyCode::Home => {
                self.fresh = false;
                self.cur = 0
            }
            KeyCode::End => {
                self.fresh = false;
                self.cur = self.text.len()
            }
            KeyCode::Backspace => {
                self.fresh = false;
                if self.cur > 0 {
                    self.cur -= 1;
                    self.text.remove(self.cur);
                }
            }
            KeyCode::Delete => {
                if self.fresh {
                    self.text.clear();
                    self.cur = 0;
                    self.fresh = false;
                } else if self.cur < self.text.len() {
                    self.text.remove(self.cur);
                }
            }
            _ => return Edit::Unhandled,
        }
        Edit::Changed
    }

    /// Draws the field in `w` cells. Fresh text uses `fresh_a`; the cursor cell is reversed.
    pub fn draw(&self, b: &mut Buffer, x: u16, y: u16, w: u16, a: u8, fresh_a: u8) {
        let w = w.max(2) as usize;
        let before: String = self.text[..self.cur].iter().collect();
        // scroll so the cursor stays visible
        let mut start = 0;
        while width(&self.text[start..self.cur].iter().collect::<String>()) >= w {
            start += 1;
        }
        let shown: String = self.text[start..].iter().collect();
        putw(b, x, y, "", w as u16, a);
        if self.fresh {
            let n = width(&shown).min(w);
            putw(b, x, y, &shown, n as u16, fresh_a);
        } else {
            put(b, x, y, &fit(&shown, w), a);
        }
        let cx = x + width(&before[before.char_indices().nth(start).map(|(i, _)| i).unwrap_or(before.len())..]) as u16;
        if cx < x + w as u16 {
            if let Some(c) = b.cell_mut((cx, y)) {
                let s = c.style();
                c.set_style(s.fg(s.bg.unwrap_or_default()).bg(crate::pal::color(15)));
            }
        }
    }
}

pub enum Modal {
    Msg { title: String, lines: Vec<String> },
    Ask { title: String, lines: Vec<String>, choices: Vec<String>, sel: usize },
    Input { title: String, lines: Vec<String>, edit: LineEdit, hist: Vec<String>, hpos: Option<usize>, w: u16 },
    Menu { title: String, items: Vec<String>, sel: usize, top: usize, w: u16 },
}

pub enum MRes {
    Keep,
    Close,
    Choice(usize),
    Text(String),
    Unhandled(KeyEvent),
}

pub fn yes_no() -> Vec<String> {
    vec![t(" 예(^Y^) ", " ^Y^es ").into(), t(" 아니오(^N^) ", " ^N^o ").into()]
}

impl Modal {
    pub fn msg(title: &str, lines: &[&str]) -> Modal {
        Modal::Msg { title: title.into(), lines: lines.iter().map(|s| s.to_string()).collect() }
    }
    pub fn ask(title: &str, lines: &[&str], choices: Vec<String>, sel: usize) -> Modal {
        Modal::Ask { title: title.into(), lines: lines.iter().map(|s| s.to_string()).collect(), choices, sel }
    }
    pub fn yes_no(title: &str, lines: &[&str], yes_default: bool) -> Modal {
        Modal::ask(title, lines, yes_no(), if yes_default { 0 } else { 1 })
    }
    pub fn input(title: &str, lines: &[&str], init: &str) -> Modal {
        Modal::Input {
            title: title.into(),
            lines: lines.iter().map(|s| s.to_string()).collect(),
            edit: LineEdit::new(init),
            hist: vec![],
            hpos: None,
            w: 60,
        }
    }
    pub fn menu(title: &str, items: Vec<String>, sel: usize) -> Modal {
        let w = items.iter().map(|s| hw(s)).max().unwrap_or(10).max(hw(title) + 4) + 4;
        Modal::Menu { title: title.into(), items, sel, top: 0, w }
    }

    pub fn key(&mut self, k: KeyEvent) -> MRes {
        match self {
            Modal::Msg { .. } => MRes::Close,
            Modal::Ask { choices, sel, .. } => {
                match k.code {
                    KeyCode::Esc => return MRes::Close,
                    KeyCode::Enter => return MRes::Choice(*sel),
                    KeyCode::Left | KeyCode::Up | KeyCode::BackTab => *sel = (*sel + choices.len() - 1) % choices.len(),
                    KeyCode::Right | KeyCode::Down | KeyCode::Tab | KeyCode::Char(' ') => *sel = (*sel + 1) % choices.len(),
                    KeyCode::Char(c) => {
                        for (i, ch) in choices.iter().enumerate() {
                            if hotkey(ch) == Some(c.to_ascii_uppercase()) {
                                return MRes::Choice(i);
                            }
                        }
                    }
                    _ => {}
                }
                MRes::Keep
            }
            Modal::Input { edit, hist, hpos, .. } => {
                match k.code {
                    KeyCode::Up | KeyCode::Down if !hist.is_empty() => {
                        let up = k.code == KeyCode::Up;
                        if edit.fresh || hpos.is_some() && edit.value() == hist[hpos.unwrap()] {
                            let n = hist.len();
                            let p = match *hpos {
                                None => if up { 0 } else { n - 1 },
                                Some(p) => if up { (p + 1) % n } else { (p + n - 1) % n },
                            };
                            *hpos = Some(p);
                            edit.set(&hist[p]);
                            edit.fresh = true;
                        } else {
                            // prefix search through the history
                            let pre = edit.value();
                            let start = hpos.map(|p| p + 1).unwrap_or(0);
                            let found = (0..hist.len()).map(|i| (start + i) % hist.len()).find(|&i| hist[i].starts_with(&pre) && hist[i] != pre);
                            if let Some(i) = found {
                                *hpos = Some(i);
                                edit.set(&hist[i]);
                            }
                        }
                        MRes::Keep
                    }
                    _ => match edit.key(&k) {
                        Edit::Done => MRes::Text(edit.value()),
                        Edit::Cancel => MRes::Close,
                        Edit::Changed => MRes::Keep,
                        Edit::Unhandled => MRes::Unhandled(k),
                    },
                }
            }
            Modal::Menu { items, sel, .. } => {
                let n = items.len();
                if n == 0 {
                    return if k.code == KeyCode::Esc { MRes::Close } else { MRes::Unhandled(k) };
                }
                match k.code {
                    KeyCode::Esc => return MRes::Close,
                    KeyCode::Enter if !ctrl(&k) && !alt(&k) => return MRes::Choice(*sel),
                    KeyCode::Up => *sel = (*sel + n - 1) % n,
                    KeyCode::Down => *sel = (*sel + 1) % n,
                    KeyCode::Home | KeyCode::PageUp => *sel = 0,
                    KeyCode::End | KeyCode::PageDown => *sel = n - 1,
                    KeyCode::Char(c) if !ctrl(&k) && !alt(&k) => {
                        let c = c.to_ascii_uppercase();
                        if let Some(i) = items.iter().position(|s| hotkey(s) == Some(c)) {
                            *sel = i;
                            return MRes::Choice(i);
                        }
                        return MRes::Unhandled(k);
                    }
                    _ => return MRes::Unhandled(k),
                }
                MRes::Keep
            }
        }
    }

    pub fn draw(&mut self, b: &mut Buffer, c: &Colors) {
        let a = c.items[DIALOG];
        let hi = hi_of(a);
        match self {
            Modal::Msg { title, lines } => {
                let w = lines.iter().map(|s| hw(s)).max().unwrap_or(0).max(hw(title) + 2).max(20) + 6;
                let (x, y) = center_box(b, w, lines.len() as u16 + 4, a, title);
                for (i, l) in lines.iter().enumerate() {
                    put_hl(b, x + 3, y + 2 + i as u16, l, a, hi);
                }
            }
            Modal::Ask { title, lines, choices, sel } => {
                let cw: u16 = choices.iter().map(|s| hw(s) + 2).sum();
                let w = lines.iter().map(|s| hw(s)).max().unwrap_or(0).max(cw).max(hw(title) + 2).max(24) + 6;
                let h = lines.len() as u16 + 5;
                let (x, y) = center_box(b, w, h, a, title);
                for (i, l) in lines.iter().enumerate() {
                    put_hl(b, x + 3, y + 1 + i as u16, l, a, hi);
                }
                let mut cx = x + (w - cw) / 2;
                let cy = y + h - 2;
                for (i, ch) in choices.iter().enumerate() {
                    let (na, ha) = if i == *sel { (0x70, 0x74) } else { (a, hi) };
                    cx = put_hl(b, cx, cy, ch, na, ha) + 2;
                }
            }
            Modal::Input { title, lines, edit, w, .. } => {
                let bw = lines.iter().map(|s| hw(s)).max().unwrap_or(0).max(*w).max(hw(title) + 2) + 6;
                let h = lines.len() as u16 + 4;
                let (x, y) = center_box(b, bw, h, a, title);
                for (i, l) in lines.iter().enumerate() {
                    put_hl(b, x + 3, y + 1 + i as u16, l, a, hi);
                }
                let fy = y + h - 2;
                edit.draw(b, x + 3, fy, bw - 6, 0x07, 0x1f);
            }
            Modal::Menu { title, items, sel, top, w } => {
                let area = b.area;
                let maxh = area.height.saturating_sub(4).max(3) as usize;
                let vis = items.len().clamp(1, maxh);
                if *sel < *top {
                    *top = *sel;
                }
                if *sel >= *top + vis {
                    *top = *sel + 1 - vis;
                }
                let (x, y) = center_box(b, *w, vis as u16 + 2, a, title);
                for (i, it) in items.iter().enumerate().skip(*top).take(vis) {
                    let yy = y + 1 + (i - *top) as u16;
                    let (na, ha) = if i == *sel { (0x70, 0x74) } else { (a, hi) };
                    putw(b, x + 1, yy, "", *w - 2, na);
                    put_hl(b, x + 2, yy, it, na, ha);
                }
            }
        }
    }
}

/// Hot key of a label: the character after the first '^', upper-cased.
pub fn hotkey(s: &str) -> Option<char> {
    s.split('^').nth(1).and_then(|h| h.chars().next()).map(|c| c.to_ascii_uppercase())
}
