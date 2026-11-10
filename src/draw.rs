//! The main screen as M.EXE draws it: function-key line, Path line, file list between two lines,
//! count line and status line (function addresses in MECHANICS.md); plus menus, help, attributes
//! and the screen saver.
use crate::act::{hints, menus};
use crate::app::{App, Mode};
use crate::list::{count_line, entry_text, file_attr, size_attr, Kind};
use mdir_core::pal::{self, Colors};
use mdir_core::ratatui::buffer::Buffer;
use mdir_core::ratatui::crossterm::event::KeyCode;
use mdir_core::scr::{center_box, fill, frame, hi_of, hline, hw, put, put_hl, putw};
use mdir_core::fsx::drive_label as fsx_label;
use mdir_core::text::{commas, fit, fmt_date, fmt_time, t, width};

/// Short name of a program definition for the function-key box ("hwp" from "c:\hwp\hwp.exe !.!").
fn short_prog(def: &str) -> String {
    let first = mdir_core::cfg::alternatives(def).into_iter().next().unwrap_or_default();
    let tok = first.replace("%M", "").replace("%m", "");
    let tok = tok.split_whitespace().next().unwrap_or("");
    let base = tok.rsplit(['/', '\\']).next().unwrap_or(tok);
    mdir_core::text::split_ext(base).0.to_lowercase()
}

/// Function-key line, row 0 (one row: the selection bar and the description are saved and redrawn
/// on row 1 by M.EXE 0x33c8, so the Path line is row 1). Cells as in M.DOC: "F2imp │3hwp  │...".
pub fn fkey_box(b: &mut Buffer, app: &App, c: &Colors) {
    let w = b.area.width;
    let a = c.items[pal::FKEY];
    let hi = c.items[pal::FKEY_HI];
    let (keys, first): (Vec<String>, u8) = match app.hint {
        Some(true) => ((1..=10).map(|i| format!("CF{}", i)).collect(), 1),
        Some(false) => ((1..=10).map(|i| format!("AF{}", i)).collect(), 1),
        None => ((2..=9).map(|i| format!("F{}", i)).collect(), 2),
    };
    putw(b, 0, 0, "", w, a);
    let cell = (w / keys.len() as u16).max(7);
    for (i, k) in keys.iter().enumerate() {
        let x = i as u16 * cell;
        let num = first as usize + i;
        let label = if i == 0 { format!("F{}", num) } else { format!("{}", num) };
        let def = app.cfg.fkeys.get(k).cloned().unwrap_or_default();
        // a key with several programs shows its name in black (MSET.DOC 펑션키 보통)
        let name_a = if mdir_core::cfg::alternatives(&def).len() > 1 { a & 0xf0 } else { a };
        if app.hint.is_none() {
            mdir_core::mouse::zone(x, 0, cell, 1, vec![mdir_core::mouse::key(KeyCode::F(num as u8))], vec![]);
        }
        let x2 = put(b, x, 0, &label, hi);
        put(b, x2, 0, &fit(&short_prog(&def), (cell as usize).saturating_sub(label.len() + 1)), name_a);
        if i + 1 < keys.len() {
            put(b, x + cell - 1, 0, "│", a);
        }
    }
}

const DAYS_KO: [&str; 7] = ["일", "월", "화", "수", "목", "금", "토"];
const DAYS_EN: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

/// Clock in the middle of the status line (M.EXE 0x212e): YY-MM-DD, day of the week, time
/// (am/pm when the 24-hour option is off).
fn clock(h24: bool) -> String {
    use mdir_core::chrono::{Datelike, Timelike};
    let n = mdir_core::chrono::Local::now();
    let day = n.weekday().num_days_from_sunday() as usize;
    let time = if h24 {
        format!("{:>2}:{:02}:{:02}", n.hour(), n.minute(), n.second())
    } else {
        let (pm, h) = n.hour12();
        format!("{:>2}:{:02}:{:02}{}", h, n.minute(), n.second(), if pm { "pm" } else { "am" })
    };
    format!(" {} {} {}", n.format("%y-%m-%d"), t(DAYS_KO[day], DAYS_EN[day]), time)
}

pub fn draw(app: &mut App, b: &mut Buffer) {
    let area = b.area;
    app.area_h = area.height;
    app.area_w = area.width;
    let c = app.cfg.colors.clone();
    if let Mode::Saver { tick } = &app.mode {
        fill(b, 0, 0, area.width, area.height, ' ', 0x00);
        let tk = *tick as u16;
        let msg = "M-dir";
        let x = (tk * 3) % area.width.saturating_sub(10).max(1);
        let y = (tk / 2) % area.height.max(1);
        put(b, x, y, msg, 0x08 | ((tk / 8) as u8 % 7 + 1));
        return;
    }
    if let Mode::Mcd(m) = &mut app.mode {
        let label = app.dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "ROOT".into());
        m.draw(b, &c, &label);
        draw_modal(app, b, &c);
        return;
    }
    let (w, h) = (area.width, area.height);
    let line = c.items[pal::LINE];
    let bg = c.items[pal::FILE] & 0xf0;
    fill(b, 0, 0, w, h, ' ', bg | 0x07);
    fkey_box(b, app, &c);

    // Path line, row 1 (seg 114b0:2730): label and path in the count-line colour, the rest of the
    // row in its highlight; path+spec cut to 52 cells; Volume [..] or Work .. ends at the right edge.
    let ca = c.items[pal::COUNT];
    let chi = c.items[pal::COUNT_HI];
    let py = 1;
    putw(b, 0, py, "", w, chi);
    let cur = app.cur_entry().cloned();
    let desc = cur.as_ref().and_then(|e| app.descs.get(&e.name).map(|s| s.to_string()));
    let text = if let Some(av) = &app.arc {
        let name = av.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let inner = if av.cwd.is_empty() { String::new() } else { format!("{}\\", av.cwd) };
        format!("{}{}{} ({})", t("압축파일 ", "Archive "), inner, name, av.kind)
    } else {
        let spec = if app.filter.is_empty() { "*.*".to_string() } else { app.filter.join(";") };
        let mut p = app.dir.display().to_string();
        if !p.ends_with(std::path::MAIN_SEPARATOR) {
            p.push(std::path::MAIN_SEPARATOR);
        }
        format!("{}{}{}", t("경로 ", "Path "), p, spec)
    };
    let text = if width(&text) > 52 { fit(&text, 52) } else { text };
    put(b, 0, py, &text, ca);
    if let (Some(d), false, true, true) = (desc, app.columns() == 1, app.cfg.num("DESCVIEW") != 2, app.arc.is_none()) {
        // description of the bar's file over the path (M.EXE 0x33c8, speed-search colour)
        putw(b, 0, py, &d, 52, c.items[pal::SEARCH]);
    }
    if let Some(s) = &app.search {
        putw(b, w.saturating_sub(24), py, &format!("[{}_]", s), 24, c.items[pal::SEARCH]);
    } else if let Some(wd) = &app.work {
        let p = fit(&wd.display().to_string(), 32).trim_end().to_string();
        let s = format!("{}{}", t(" 작업 ", " Work "), p);
        put(b, w.saturating_sub(width(&s) as u16), py, &s, chi);
    } else {
        let vol = mdir_core::fsx::drive_of(&app.dir, &app.drives).map(|d| d.root.file_name().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_else(|| "ROOT".into())).unwrap_or_default();
        let s = format!("{}{}]", t("볼륨명 [", "Volume ["), fit(&vol, 11).trim_end());
        put(b, w.saturating_sub(width(&s) as u16), py, &s, chi);
    }

    // file list: rows 3.. between two lines (or rows 2.. without them)
    let rows = app.list_rows();
    let cols = app.columns();
    let top_y = 2u16;
    let list_y = if app.hline { 3u16 } else { 2u16 };
    let bot_y = list_y + rows as u16;
    let vert = app.cfg.flag("VERTLINE");
    let sbar = app.cfg.flag("SCROLLBAR");
    let lw = if sbar { w - 1 } else { w } as usize;
    let cw = lw.saturating_sub(cols - 1) / cols;
    if app.hline {
        hline(b, 0, top_y, w, '─', line);
        hline(b, 0, bot_y, w, '─', line);
    }
    let ps = rows * cols;
    let first = app.cur / ps * ps;
    let fmt = app.fmt();
    let mode1 = cols == 1;
    for col in 0..cols {
        let x0 = (col * (cw + 1)) as u16;
        if col > 0 && vert {
            let sx = x0 - 1;
            if app.hline {
                put(b, sx, top_y, "┬", line);
                put(b, sx, bot_y, "┴", line);
            }
            fill(b, sx, list_y, 1, rows as u16, '│', line);
        }
        for r in 0..rows {
            let i = first + col * rows + r;
            let y = list_y + r as u16;
            let Some(e) = app.entries.get(i) else { continue };
            mdir_core::mouse::zone(x0, y, cw as u16, 1, mdir_core::mouse::move_bar(app.cur, i), vec![mdir_core::mouse::key(KeyCode::Enter)]);
            let desc = if mode1 && app.cfg.num("DESCVIEW") != 2 { app.descs.get(&e.name) } else { None };
            let order = if mode1 && e.tag > 0 { Some(order_of(app, i)) } else { None };
            let text = entry_text(e, cw, &fmt, desc, order);
            let mut a = file_attr(e, &c);
            if e.tag > 0 {
                a = 0x10 | (a & 0x0f).max(0x07);
            }
            if i == app.cur {
                put(b, x0, y, &text, pal::bar(a));
                continue;
            }
            // name in its own colour, size/date/time in the "other files" colour (seg 114b0:0633)
            let rest_a = if e.tag > 0 { a } else { c.items[pal::FILE] };
            let name_w = if matches!(e.kind, Kind::Drive | Kind::Exit) { cw } else { 12.min(cw) };
            put(b, x0, y, &text, rest_a);
            put(b, x0, y, &fit(&text, name_w), a);
            if e.tag == 0 && e.is_file() && cw >= 19 {
                let n = if cw >= 24 { 11 } else { 7 };
                let sx = x0 + 12 + u16::from(cw >= 24 && cw < 38);
                let sz = crate::list::size_field(e, n);
                put(b, sx, y, &sz, size_attr(e, &c));
                if sz.ends_with('M') {
                    // the M of a large size is bright red
                    put(b, sx + n as u16 - 1, y, "M", (c.items[pal::FILE] & 0xf0) | 0x0c);
                }
            }
        }
    }
    if sbar {
        // scroll bar (M.EXE 0x300a): track 0xB0, thumb 0x88 in colour
        let x = w - 1;
        let total = app.entries.len().max(1);
        let len = (rows * ps / total).clamp(1, rows);
        let pos = (first * rows / total).min(rows - len);
        for r in 0..rows {
            let on = r >= pos && r < pos + len;
            put(b, x, list_y + r as u16, if on { " " } else { "░" }, if on { 0x88 } else { 0xb0 });
        }
    }

    // count line (seg 114b0:29bf)
    let cy = if app.hline { bot_y + 1 } else { bot_y };
    putw(b, 0, cy, "", w, ca);
    let files = app.entries.iter().filter(|e| e.kind == Kind::File).count();
    let dirs = app.entries.iter().filter(|e| e.kind == Kind::Dir).count();
    let bytes: u64 = app.entries.iter().filter(|e| e.kind == Kind::File).map(|e| e.size).sum();
    put_hl(b, 0, cy, &count_line(files, dirs, bytes, if app.arc.is_some() { None } else { app.space }), ca, chi);
    // sort letter at column 79: its own colour per sort (+9 ascending, +0 descending); H at 80
    if let Some(l) = app.sort.letter() {
        let k = app.sort.index() as u8;
        let fg = if app.desc { k } else { k + 9 };
        put(b, w.saturating_sub(2), cy, &l.to_string(), (ca & 0xf0) | (fg & 0x0f));
    }
    if app.hidden {
        put(b, w.saturating_sub(1), cy, "H", (chi & 0xf0) | 0x0a);
    }

    // status line, last row (M.EXE 0x26f0): size│date│time│attr│clock or selection│F10│F11│F12
    let sy = h - 1;
    let a = c.items[pal::FKEY];
    let hi = c.items[pal::FKEY_HI];
    putw(b, 0, sy, "", w, a);
    if let Some(ct) = app.hint {
        put_hl(b, 0, sy, &fit_hl(hints(ct), w as usize), a, hi);
    } else if let Some(m) = &app.msg {
        put(b, 1, sy, &fit(m, w as usize - 2), a);
    } else {
        let h24 = app.cfg.flag("24HOUR");
        let (size, date, time, attr) = match &cur {
            Some(e) if e.kind == Kind::Drive => (format!("{}{}:", t("드라이브", " Drive "), e.drive.as_ref().map(|d| fsx_label(d)).unwrap_or_default()), String::new(), String::new(), String::new()),
            Some(e) if e.is_dirlike() => (t(" 디렉토리", "Directory").to_string(), fmt_date(e.mtime), fmt_time(e.mtime, h24), e.attr.text()),
            Some(e) if e.kind == Kind::File => (if e.size > 999_999_999 { format!("{:>8}M", e.size >> 20) } else { format!("{:>9}", commas(e.size)) }, fmt_date(e.mtime), fmt_time(e.mtime, h24), e.attr.text()),
            _ => (String::new(), String::new(), String::new(), String::new()),
        };
        let left = format!("{}│{}│{}│{}│", fit(&size, 9), fit(&date, 8), fit(&time, 5), fit(&attr, 4));
        let x = put(b, 0, sy, &left, a);
        let right = format!("│^F10^=MCD│^F11^=QCD│^F12^={}", t("메뉴", "Menu"));
        let rx = w.saturating_sub(hw(&right));
        let tagged: Vec<&crate::list::Entry> = app.entries.iter().filter(|e| e.tag > 0).collect();
        let mid = if tagged.is_empty() {
            clock(h24)
        } else {
            format!("{:>4} File{:>11} Byte", tagged.len(), commas(tagged.iter().map(|e| e.size).sum()))
        };
        put(b, x, sy, &fit(&mid, rx.saturating_sub(x) as usize), if tagged.is_empty() { a } else { hi });
        put_hl(b, rx, sy, &right, a, hi);
        for (k, (dx, dw)) in [(0u16, 8u16), (8, 8), (16, hw(&right).saturating_sub(16))].into_iter().enumerate() {
            mdir_core::mouse::zone(rx + dx, sy, dw, 1, vec![mdir_core::mouse::key(KeyCode::F(10 + k as u8))], vec![]);
        }
    }

    match &app.mode {
        Mode::Menu { m, i } => draw_menu(b, app, &c, *m, *i),
        Mode::Help { lines, top } => draw_help(b, &c, lines, *top),
        Mode::Attr { names, a } => draw_attr(b, &c, names, *a),
        _ => {}
    }
    draw_modal(app, b, &c);
}

fn order_of(app: &App, i: usize) -> u32 {
    let tg = app.entries[i].tag;
    app.entries.iter().filter(|e| e.tag > 0 && e.tag <= tg).count() as u32
}

fn fit_hl(s: &str, w: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        if ch == '^' {
            out.push(ch);
            continue;
        }
        let cw = width(&ch.to_string());
        if used + cw > w {
            break;
        }
        out.push(ch);
        used += cw;
    }
    out
}

fn draw_modal(app: &mut App, b: &mut Buffer, c: &Colors) {
    if let Some((m, _)) = app.modal.as_mut() {
        m.draw(b, c);
    }
}

fn draw_menu(b: &mut Buffer, app: &App, c: &Colors, m: usize, i: usize) {
    let w = b.area.width;
    let a = c.items[pal::DIALOG];
    let hi = hi_of(a);
    let ms = menus();
    let ar = b.area;
    mdir_core::mouse::block(ar.x, ar.y, ar.width, ar.height, vec![mdir_core::mouse::key(KeyCode::Esc)]);
    putw(b, 0, 0, "", w, a);
    let mut x = 1;
    let mut xs = vec![];
    for (k, md) in ms.iter().enumerate() {
        let title = t(md.ko, md.en);
        xs.push(x);
        let (na, ha) = if k == m { (0x70, 0x74) } else { (a, hi) };
        let x0 = x;
        x = put_hl(b, x, 0, title, na, ha) + 1;
        if let Some(l) = "MRFDAO".chars().nth(k) {
            mdir_core::mouse::zone(x0, 0, x - x0, 1, vec![mdir_core::mouse::key(KeyCode::Char(l))], vec![]);
        }
    }
    let items = &ms[m].items;
    let iw = items.iter().map(|it| hw(t(it.ko, it.en))).max().unwrap_or(10) + 6;
    let x0 = xs[m].min(w.saturating_sub(iw + 2));
    frame(b, x0, 1, iw, items.len() as u16 + 2, a, false);
    mdir_core::scr::shadow(b, x0, 1, iw, items.len() as u16 + 2);
    for (k, it) in items.iter().enumerate() {
        let y = 2 + k as u16;
        let mut label = t(it.ko, it.en).to_string();
        match it.act {
            crate::act::Act::GraphicChar => label += mdir_core::cfg::HANGUL[app.cfg.num("HANGUL").clamp(0, 3) as usize].1,
            crate::act::Act::CycleColumns => label += &if app.cols_mode == 0 { t("자동", "Auto").to_string() } else { app.cols_mode.to_string() },
            crate::act::Act::CycleSort => label += t(mdir_core::cfg::SORTS[app.sort.index() as usize].0, mdir_core::cfg::SORTS[app.sort.index() as usize].1),
            _ => {}
        }
        let (na, ha) = if k == i { (0x70, 0x74) } else { (a, hi) };
        putw(b, x0 + 1, y, "", iw - 2, na);
        put_hl(b, x0 + 2, y, &label, na, ha);
        let mut ks = vec![mdir_core::mouse::key(KeyCode::Home)];
        ks.extend(mdir_core::mouse::times(KeyCode::Down, k));
        ks.push(mdir_core::mouse::key(KeyCode::Enter));
        mdir_core::mouse::zone(x0 + 1, y, iw - 2, 1, ks, vec![]);
    }
}

fn draw_help(b: &mut Buffer, c: &Colors, lines: &[String], top: usize) {
    let area = b.area;
    let a = c.items[pal::HELP];
    let hi = (a & 0xf0) | 0x0f;
    let wd = (lines.iter().map(|l| hw(l)).max().unwrap_or(40) + 4).min(area.width);
    let ht = (lines.len() as u16 + 2).min(area.height);
    mdir_core::mouse::block(area.x, area.y, area.width, area.height, vec![mdir_core::mouse::key(KeyCode::Esc)]);
    let (x, y) = center_box(b, wd, ht, a, t("도움말", "Quick Help"));
    for (k, l) in lines.iter().skip(top).take(ht as usize - 2).enumerate() {
        put_hl(b, x + 2, y + 1 + k as u16, l, a, hi);
    }
}

fn draw_attr(b: &mut Buffer, c: &Colors, names: &[String], at: mdir_core::fsx::Attr) {
    let a = c.items[pal::DIALOG];
    let hi = hi_of(a);
    let (x, y) = center_box(b, 44, 10, a, t("속성바꾸기", "Change Attributes"));
    put(b, x + 3, y + 1, &fit(&names[0], 38), a);
    let rows = [
        (t("(^A^)변경   [", "^A^rchive   ["), at.archive),
        (t("(^R^)읽기만 [", "^R^eadOnly  ["), at.readonly),
        (t("(^H^)숨김   [", "^H^idden    ["), at.hidden),
        (t("(^S^)시스템 [", "^S^ystem    ["), at.system),
    ];
    for (k, (l, on)) in rows.iter().enumerate() {
        let xx = put_hl(b, x + 6, y + 3 + k as u16, l, a, hi);
        put(b, xx, y + 3 + k as u16, if *on { "√]" } else { " ]" }, a);
    }
    put_hl(b, x + 3, y + 8, t("^Enter^-저장    ^End^-전부저장", "^Enter^-Change  ^End^-Change All"), a, hi);
}
