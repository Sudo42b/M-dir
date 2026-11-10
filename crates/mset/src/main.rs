//! MSET — MdirIII 3.10 setup: six pull-down menus (Mdir, 프로그램, 메인화면, MCD, 기타, 색깔),
//! extension and function-key program lists, colour picker, and a preview of the main screen.
use mdir_core::cfg::{fkey_label, fkey_names, Config, Kind, OPTIONS};
use mdir_core::modal::{alt, ctrl, MRes, Modal};
use mdir_core::pal::{self, Colors, ITEM_NAMES};
use mdir_core::ratatui::buffer::Buffer;
use mdir_core::ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mdir_core::ratatui::layout::Rect;
use mdir_core::scr::{fill, frame, hi_of, hw, put, put_hl, putw, screen_text, shadow};
use mdir_core::term::{self, k, Flow, Report, Tui};
use mdir_core::text::{fit, t};
use std::path::Path;

const MENUS: [(&str, &str); 6] = [("Mdir", "Mdir"), ("프로그램", "Program"), ("메인화면", "Main"), ("MCD", "MCD"), ("기타", "Etc"), ("색깔", "Color")];

#[derive(Clone)]
enum Item {
    Cmd(&'static str, &'static str, u8),
    Opt(&'static str),
    Color(usize),
}

fn items(m: usize) -> Vec<Item> {
    let mut v = match m {
        0 => vec![Item::Cmd("메뉴숨김", "Hide menu", 0), Item::Cmd("도움말", "Help", 1), Item::Cmd("불러오기", "Load", 2), Item::Cmd("저장하기", "Save", 3), Item::Cmd("색깔부르기", "Load colors", 4), Item::Cmd("색깔저장", "Save colors", 5), Item::Cmd("등록판생성", "Register", 6), Item::Cmd("종료", "Quit", 7)],
        1 => vec![Item::Cmd("확장자지정", "Extensions", 10), Item::Cmd("펑션키지정", "Function keys", 11)],
        5 => vec![Item::Cmd("파일확장자별", "By extension", 20)],
        _ => vec![],
    };
    if (1..=4).contains(&m) {
        v.extend(OPTIONS.iter().filter(|o| o.menu as usize == m + 1).map(|o| Item::Opt(o.key)));
    }
    if m == 5 {
        v.extend((0..14).map(Item::Color));
        v.push(Item::Cmd("메인화면배경", "Main background", 21));
        v.push(Item::Cmd("펑션키 배경", "Fkey background", 22));
        v.push(Item::Cmd("팔레트", "Palette", 23));
    }
    v
}

enum P {
    Info,
    Opt(&'static str),
    Choice(&'static str),
    ExtName(Option<usize>),
    ExtProg(usize),
    Fkey(String),
    ColLoad,
    ColPath(bool),
    ColorExtName(Option<usize>),
    Quit,
}

enum Ed {
    Ext(usize),
    Fkey(usize),
    ExtColor(usize),
    /// item index (or 100+i for extension colours, 200 main bg, 201 fkey bg), attribute, background only
    Pick(usize, u8, bool),
}

struct Mset {
    cfg: Config,
    m: usize,
    i: usize,
    hidden: bool,
    modal: Option<(Modal, P)>,
    ed: Option<Ed>,
    clip: Option<(String, String)>,
    back_ed: Option<Ed>,
}

fn opt_value(cfg: &Config, key: &str) -> String {
    let o = mdir_core::cfg::opt(key).unwrap();
    let v = cfg.get(key);
    match o.kind {
        Kind::Bool => if cfg.flag(key) { "o".into() } else { "x".into() },
        Kind::Choice(list) => {
            let i = v.parse::<usize>().unwrap_or(0).min(list.len() - 1);
            t(list[i].0, list[i].1).to_string()
        }
        _ => v,
    }
}

impl Mset {
    fn label(&self, it: &Item) -> String {
        match it {
            Item::Cmd(ko, en, _) => t(ko, en).to_string(),
            Item::Opt(k) => {
                let o = mdir_core::cfg::opt(k).unwrap();
                format!("{}{}", fit(t(o.ko, o.en), 15), fit(&opt_value(&self.cfg, k), 14).trim_end())
            }
            Item::Color(i) => format!("{}{:02X}", fit(t(ITEM_NAMES[*i].0, ITEM_NAMES[*i].1), 15), self.cfg.colors.items[*i]),
        }
    }

    fn help_line(&self) -> String {
        let its = items(self.m);
        match its.get(self.i) {
            Some(Item::Opt(k)) => {
                let o = mdir_core::cfg::opt(k).unwrap();
                t(o.help_ko, o.help_en).to_string()
            }
            Some(Item::Color(i)) => format!("{} {}", t(ITEM_NAMES[*i].0, ITEM_NAMES[*i].1), t("색깔을 지정합니다", "colour")),
            Some(Item::Cmd(_, _, n)) => match n {
                0 => t("현재 설정상태를 볼 수 있도록 메뉴를 숨깁니다 (아무키나 누르면 복귀)", "Hide the menu to see the settings"),
                1 => t("MSET의 사용법", "How to use MSET"),
                2 => t("M.CFG를 읽어 들입니다", "Read M.CFG"),
                3 => t("M.CFG를 디스크에 저장합니다", "Write M.CFG"),
                4 => t("색깔파일(*.COL)을 읽습니다. BLACK.COL이 초기치입니다", "Read a colour file (*.COL); BLACK.COL is the default"),
                5 => t("색깔만 별도 파일로 저장합니다", "Write the colours to a file"),
                6 => t("등록판 생성 (이 판에는 필요하지 않습니다)", "Make a registered copy (not needed here)"),
                7 => t("MSET을 끝냅니다", "Quit MSET"),
                10 => t("확장자별 응용프로그램 지정 (엔터키로 실행)", "Programs per extension (run with Enter)"),
                11 => t("펑션키별 응용프로그램 지정 (F2-F9, Ctrl/Alt-F1..F10)", "Programs on F2-F9, Ctrl/Alt-F1..F10"),
                20 => t("파일 확장자별 색깔 (최대 146개)", "Colours per extension"),
                21 => t("파일목록의 배경색을 한 번에 지정합니다", "Background of the whole file list"),
                22 => t("펑션키와 상태선의 배경색", "Background of the function keys and status lines"),
                23 => t("VGA에서만 가능합니다 (터미널에서는 쓰지 않음)", "VGA only (not used in a terminal)"),
                _ => "",
            }
            .to_string(),
            None => String::new(),
        }
    }

    fn quit(&mut self) -> Flow {
        if self.cfg.dirty {
            self.modal = Some((Modal::yes_no("MSET", &[t("환경이 바뀌었습니다. 저장할까요?", "Settings changed. Save?")], true), P::Quit));
            return Flow::Cont;
        }
        Flow::Quit
    }

    fn info(&mut self, title: &str, lines: &[&str]) {
        self.modal = Some((Modal::msg(title, lines), P::Info));
    }

    fn activate(&mut self) -> Flow {
        let its = items(self.m);
        let Some(it) = its.get(self.i).cloned() else { return Flow::Cont };
        match it {
            Item::Opt(key) => {
                let o = mdir_core::cfg::opt(key).unwrap();
                match o.kind {
                    Kind::Bool => {
                        let v = !self.cfg.flag(key);
                        self.cfg.set_flag(key, v);
                    }
                    Kind::Choice(list) => {
                        let cur = self.cfg.num(key).max(0) as usize;
                        let m = Modal::menu(t(o.ko, o.en), list.iter().map(|(ko, en)| format!(" {} ", t(ko, en))).collect(), cur.min(list.len() - 1));
                        self.modal = Some((m, P::Choice(key)));
                    }
                    _ => {
                        let mut m = Modal::input(t(o.ko, o.en), &[t(o.help_ko, o.help_en)], &self.cfg.get(key));
                        if let Modal::Input { w, .. } = &mut m {
                            *w = 56;
                        }
                        self.modal = Some((m, P::Opt(key)));
                    }
                }
            }
            Item::Color(i) => self.ed = Some(Ed::Pick(i, self.cfg.colors.items[i], false)),
            Item::Cmd(_, _, n) => match n {
                0 => self.hidden = true,
                1 => self.info(t("도움말", "Help"), &[
                    t("^←^^→^^↑^^↓^ 메뉴 이동   ^Enter^ 선택   ^Esc^ 이전단계", "^Arrows^ move   ^Enter^ select   ^Esc^ back"),
                    t("^1^..^6^ 주메뉴로   ^Tab^ 메뉴숨김   ^Alt-X^ 종료", "^1^..^6^ main menus   ^Tab^ hide menu   ^Alt-X^ quit"),
                    t("목록에서: ^Space^ 확장자 ^Enter^ 프로그램 ^Del^ 삭제 ^Ins^ 추가 ^^C^ 오려두기 ^^P^ 붙이기", "Lists: ^Space^ ext ^Enter^ program ^Del^ delete ^Ins^ add ^^C^ copy ^^P^ paste"),
                ]),
                2 => {
                    let p = self.cfg.path.clone();
                    self.cfg = Config::load_from(&p);
                    self.info("MSET", &[t("M.CFG를 읽었습니다", "M.CFG loaded")]);
                }
                3 => {
                    let ok = self.cfg.save().is_ok();
                    self.cfg.dirty = !ok;
                    let p = self.cfg.path.display().to_string();
                    self.info("MSET", &[if ok { t("저장했습니다", "Saved") } else { t("저장할 수 없습니다", "Cannot save") }, &p]);
                }
                4 => {
                    let m = Modal::menu(t("색깔부르기", "Load colors"), vec![t(" 검정 바탕 ", " Black ").into(), t(" 파랑 바탕 ", " Blue ").into(), t(" 파일에서... ", " From file... ").into()], 0);
                    self.modal = Some((m, P::ColLoad));
                }
                5 => {
                    let def = term::data_dir().join("M.COL").display().to_string();
                    self.modal = Some((Modal::input(t("색깔저장", "Save colors"), &[t("저장할 색깔파일 이름", "Colour file name")], &def), P::ColPath(true)));
                }
                6 => self.info(t("등록판생성", "Register"), &[t("이 판은 등록 없이 모든 기능을 씁니다", "Every feature works without registering")]),
                7 => return self.quit(),
                10 => self.ed = Some(Ed::Ext(0)),
                11 => self.ed = Some(Ed::Fkey(0)),
                20 => self.ed = Some(Ed::ExtColor(0)),
                21 => self.ed = Some(Ed::Pick(200, self.cfg.colors.items[pal::FILE], true)),
                22 => self.ed = Some(Ed::Pick(201, self.cfg.colors.items[pal::FKEY], true)),
                _ => self.info(t("팔레트", "Palette"), &[t("VGA에서만 가능합니다", "VGA only")]),
            },
        }
        Flow::Cont
    }

    fn list_key(&mut self, kk: KeyEvent) {
        let Some(ed) = self.ed.take() else { return };
        let c = ctrl(&kk);
        let next = match ed {
            Ed::Ext(sel) => {
                let n = self.cfg.ext.len();
                match kk.code {
                    KeyCode::Esc => None,
                    KeyCode::Up => Some(Ed::Ext(sel.saturating_sub(1))),
                    KeyCode::Down => Some(Ed::Ext((sel + 1).min(n.saturating_sub(1)))),
                    KeyCode::Home | KeyCode::PageUp => Some(Ed::Ext(0)),
                    KeyCode::End | KeyCode::PageDown => Some(Ed::Ext(n.saturating_sub(1))),
                    KeyCode::Char(' ') if sel < n => {
                        self.modal = Some((Modal::input(t("확장자", "Extension"), &[t("확장자 (예: HWP)", "Extension (e.g. HWP)")], &self.cfg.ext[sel].0), P::ExtName(Some(sel))));
                        Some(Ed::Ext(sel))
                    }
                    KeyCode::Enter if sel < n => {
                        let mut m = Modal::input(&format!(".{}", self.cfg.ext[sel].0), &[t("프로그램 (예: hwp !.!)  변수: !.! %F !\\ %P %% ...", "Program (e.g. hwp !.!)  vars: !.! %F !\\ %P %% ...")], &self.cfg.ext[sel].1);
                        if let Modal::Input { w, .. } = &mut m {
                            *w = 60;
                        }
                        self.modal = Some((m, P::ExtProg(sel)));
                        Some(Ed::Ext(sel))
                    }
                    KeyCode::Insert => {
                        if n < 100 {
                            self.modal = Some((Modal::input(t("확장자 추가", "Add extension"), &[t("확장자 (예: HWP)", "Extension (e.g. HWP)")], ""), P::ExtName(None)));
                        }
                        Some(Ed::Ext(sel))
                    }
                    KeyCode::Delete if sel < n => {
                        self.clip = Some(self.cfg.ext.remove(sel));
                        self.cfg.dirty = true;
                        Some(Ed::Ext(sel.min(n.saturating_sub(2))))
                    }
                    KeyCode::Char('c') if c && sel < n => {
                        self.clip = Some(self.cfg.ext[sel].clone());
                        Some(Ed::Ext(sel))
                    }
                    KeyCode::Char('p') if c => {
                        if let Some(x) = self.clip.clone() {
                            if !self.cfg.ext.iter().any(|e| e.0 == x.0) {
                                self.cfg.ext.insert(sel.min(n), x);
                                self.cfg.dirty = true;
                            }
                        }
                        Some(Ed::Ext(sel))
                    }
                    _ => Some(Ed::Ext(sel)),
                }
            }
            Ed::Fkey(sel) => {
                let names = fkey_names();
                let n = names.len();
                let key = names[sel].clone();
                match kk.code {
                    KeyCode::Esc => None,
                    KeyCode::Up => Some(Ed::Fkey(sel.saturating_sub(1))),
                    KeyCode::Down => Some(Ed::Fkey((sel + 1).min(n - 1))),
                    KeyCode::Home | KeyCode::PageUp => Some(Ed::Fkey(0)),
                    KeyCode::End | KeyCode::PageDown => Some(Ed::Fkey(n - 1)),
                    KeyCode::Enter => {
                        let cur = self.cfg.fkeys.get(&key).cloned().unwrap_or_default();
                        let mut m = Modal::input(&fkey_label(&key), &[t("프로그램 (여러개는 ;로 구분)", "Program (several: separate with ;)")], &cur);
                        if let Modal::Input { w, .. } = &mut m {
                            *w = 60;
                        }
                        self.modal = Some((m, P::Fkey(key)));
                        Some(Ed::Fkey(sel))
                    }
                    KeyCode::Delete => {
                        if let Some(v) = self.cfg.fkeys.remove(&key) {
                            self.clip = Some((key, v));
                            self.cfg.dirty = true;
                        }
                        Some(Ed::Fkey(sel))
                    }
                    KeyCode::Char('c') if c => {
                        self.clip = self.cfg.fkeys.get(&key).map(|v| (key.clone(), v.clone()));
                        Some(Ed::Fkey(sel))
                    }
                    KeyCode::Char('p') if c => {
                        if let Some((_, v)) = self.clip.clone() {
                            self.cfg.fkeys.insert(key, v);
                            self.cfg.dirty = true;
                        }
                        Some(Ed::Fkey(sel))
                    }
                    _ => Some(Ed::Fkey(sel)),
                }
            }
            Ed::ExtColor(sel) => {
                let n = self.cfg.colors.ext.len();
                match kk.code {
                    KeyCode::Esc => None,
                    KeyCode::Up => Some(Ed::ExtColor(sel.saturating_sub(1))),
                    KeyCode::Down => Some(Ed::ExtColor((sel + 1).min(n.saturating_sub(1)))),
                    KeyCode::Home | KeyCode::PageUp => Some(Ed::ExtColor(0)),
                    KeyCode::End | KeyCode::PageDown => Some(Ed::ExtColor(n.saturating_sub(1))),
                    KeyCode::Enter if sel < n => {
                        self.back_ed = Some(Ed::ExtColor(sel));
                        Some(Ed::Pick(100 + sel, self.cfg.colors.ext[sel].1, false))
                    }
                    KeyCode::Char(' ') if sel < n && sel >= 4 => {
                        self.modal = Some((Modal::input(t("확장자", "Extension"), &[""], &self.cfg.colors.ext[sel].0), P::ColorExtName(Some(sel))));
                        Some(Ed::ExtColor(sel))
                    }
                    KeyCode::Insert => {
                        if n < 150 {
                            self.modal = Some((Modal::input(t("확장자 추가", "Add extension"), &[""], ""), P::ColorExtName(None)));
                        }
                        Some(Ed::ExtColor(sel))
                    }
                    // the four executable extensions (EXE COM BAT BTM) cannot be removed or renamed
                    KeyCode::Delete if sel < n && sel >= 4 => {
                        let x = self.cfg.colors.ext.remove(sel);
                        self.clip = Some((x.0, format!("{:02X}", x.1)));
                        self.cfg.dirty = true;
                        Some(Ed::ExtColor(sel.min(n.saturating_sub(2))))
                    }
                    KeyCode::Char('c') if c && sel < n => {
                        let x = &self.cfg.colors.ext[sel];
                        self.clip = Some((x.0.clone(), format!("{:02X}", x.1)));
                        Some(Ed::ExtColor(sel))
                    }
                    KeyCode::Char('p') if c => {
                        if let Some((e, a)) = self.clip.clone() {
                            if let Ok(a) = u8::from_str_radix(&a, 16) {
                                if !self.cfg.colors.ext.iter().any(|x| x.0 == e) {
                                    self.cfg.colors.ext.insert(sel.min(n).max(4), (e, a));
                                    self.cfg.dirty = true;
                                }
                            }
                        }
                        Some(Ed::ExtColor(sel))
                    }
                    _ => Some(Ed::ExtColor(sel)),
                }
            }
            Ed::Pick(what, a, bg_only) => {
                let (mut fg, mut bg) = (a & 15, a >> 4);
                match kk.code {
                    KeyCode::Esc => {
                        self.ed = self.back_ed.take();
                        return;
                    }
                    KeyCode::Enter => {
                        let a = (bg << 4) | fg;
                        let cs = &mut self.cfg.colors;
                        match what {
                            200 => {
                                for i in [pal::DIR, pal::DRIVE, pal::LINE, pal::FILE] {
                                    cs.items[i] = (cs.items[i] & 0x0f) | (bg << 4);
                                }
                            }
                            201 => {
                                for i in [pal::FKEY, pal::FKEY_HI] {
                                    cs.items[i] = (cs.items[i] & 0x0f) | (bg << 4);
                                }
                            }
                            w if w >= 100 => cs.ext[w - 100].1 = a,
                            w => cs.items[w] = a,
                        }
                        self.cfg.dirty = true;
                        self.ed = self.back_ed.take();
                        return;
                    }
                    KeyCode::Left if !bg_only => fg = (fg + 15) % 16,
                    KeyCode::Right if !bg_only => fg = (fg + 1) % 16,
                    KeyCode::Up => bg = (bg + 15) % 16,
                    KeyCode::Down => bg = (bg + 1) % 16,
                    _ => {}
                }
                Some(Ed::Pick(what, (bg << 4) | fg, bg_only))
            }
        };
        self.ed = next;
    }

    fn modal_done(&mut self, p: P, r: MRes) -> Flow {
        match (p, r) {
            (P::Opt(key), MRes::Text(s)) => {
                let o = mdir_core::cfg::opt(key).unwrap();
                if let Kind::Num(lo, hi) = o.kind {
                    match s.trim().parse::<i64>() {
                        Ok(n) if n >= lo && n <= hi => self.cfg.set(key, &n.to_string()),
                        _ => self.info("MSET", &[&format!("{} {}..{}", t("범위:", "Range:"), lo, hi)]),
                    }
                } else {
                    self.cfg.set(key, s.trim());
                }
            }
            (P::Choice(key), MRes::Choice(i)) => self.cfg.set(key, &i.to_string()),
            (P::ExtName(idx), MRes::Text(s)) => {
                let e = s.trim().trim_start_matches('.').to_ascii_uppercase();
                if !e.is_empty() {
                    match idx {
                        Some(i) => self.cfg.ext[i].0 = e,
                        None => {
                            if self.cfg.ext.iter().any(|x| x.0 == e) {
                                self.info("MSET", &[t("이미 정의되어 있습니다", "Already defined")]);
                            } else {
                                self.cfg.ext.push((e, String::new()));
                                let i = self.cfg.ext.len() - 1;
                                self.ed = Some(Ed::Ext(i));
                                self.modal = Some((Modal::input(&format!(".{}", self.cfg.ext[i].0), &[t("프로그램 (예: hwp !.!)", "Program (e.g. hwp !.!)")], ""), P::ExtProg(i)));
                            }
                        }
                    }
                    self.cfg.dirty = true;
                }
            }
            (P::ExtProg(i), MRes::Text(s)) => {
                self.cfg.ext[i].1 = s.trim().to_string();
                self.cfg.dirty = true;
            }
            (P::Fkey(key), MRes::Text(s)) => {
                if s.trim().is_empty() {
                    self.cfg.fkeys.remove(&key);
                } else {
                    self.cfg.fkeys.insert(key, s.trim().to_string());
                }
                self.cfg.dirty = true;
            }
            (P::ColorExtName(idx), MRes::Text(s)) => {
                let e = s.trim().trim_start_matches('.').to_ascii_uppercase();
                if !e.is_empty() {
                    match idx {
                        Some(i) => self.cfg.colors.ext[i].0 = e,
                        None => self.cfg.colors.ext.push((e, 0x07)),
                    }
                    self.cfg.dirty = true;
                }
            }
            (P::ColLoad, MRes::Choice(i)) => match i {
                0 => {
                    self.cfg.colors = Colors::black();
                    self.cfg.dirty = true;
                }
                1 => {
                    self.cfg.colors = Colors::blue();
                    self.cfg.dirty = true;
                }
                _ => self.modal = Some((Modal::input(t("색깔부르기", "Load colors"), &[t("색깔파일 이름 (*.COL)", "Colour file (*.COL)")], ""), P::ColPath(false))),
            },
            (P::ColPath(save), MRes::Text(s)) => {
                let p = Path::new(s.trim());
                if save {
                    let ok = std::fs::write(p, self.cfg.colors.write()).is_ok();
                    self.info("MSET", &[if ok { t("색깔을 저장했습니다", "Colours saved") } else { t("저장할 수 없습니다", "Cannot save") }]);
                } else {
                    match std::fs::read(p).ok().and_then(|d| Colors::parse(&d)) {
                        Some(c) => {
                            self.cfg.colors = c;
                            self.cfg.dirty = true;
                        }
                        None => self.info("MSET", &[t("색깔파일이 아닙니다", "Not a colour file")]),
                    }
                }
            }
            (P::Quit, MRes::Choice(c)) => {
                if c == 0 {
                    let _ = self.cfg.save();
                }
                return Flow::Quit;
            }
            _ => {}
        }
        Flow::Cont
    }
}

/// Small picture of the main screen and MCD in the current colours (left side, like MSET's).
fn preview(b: &mut Buffer, c: &Colors, x0: u16, y0: u16) {
    let w = 36u16;
    let bg = c.items[pal::FILE] & 0xf0;
    let fk = c.items[pal::FKEY];
    let fh = c.items[pal::FKEY_HI];
    fill(b, x0, y0, w, 13, ' ', bg | 7);
    putw(b, x0, y0, "", w, fk);
    put_hl(b, x0, y0, "^F2^HWP ^3^QEDIT ^4^V3+ ^5^IYG ^6^SAN ^7^PcT", fk, fh);
    put(b, x0, y0 + 1, &fit("Path C:\\DOS\\*.*", 26), (bg) | (c.items[pal::LINE] & 15));
    putw(b, x0 + 26, y0 + 1, "[TEST    ]", 10, c.items[pal::SEARCH]);
    mdir_core::scr::hline(b, x0, y0 + 2, w, '═', c.items[pal::LINE]);
    let ext = |e: &str| c.ext_color(e).map(|a| bg | (a & 15)).unwrap_or(c.items[pal::FILE]);
    let rows: [(&str, u8); 8] = [
        ("TEST          DIR  ", c.items[pal::DIR]),
        ("APPEND   EXE  1234 ", ext("EXE")),
        ("COMMAND  COM   123 ", ext("COM")),
        ("AUTOEXEC BAT  2345 ", ext("BAT")),
        ("4DOS     BTM   345 ", ext("BTM")),
        ("NORMAL   XYZ    10 ", c.items[pal::FILE]),
        ("READONLY EXE   345 ", ext("EXE")),
        ("HIDDEN   COM    10 ", bg | 0x05),
    ];
    for (i, (s, a)) in rows.iter().enumerate() {
        put(b, x0, y0 + 3 + i as u16, s, *a);
        if i == 6 {
            put(b, x0 + 13, y0 + 3 + i as u16, "  345 ", bg | 0x0e);
        }
    }
    put(b, x0 + 20, y0 + 3, "[-A-] 3.5\"", c.items[pal::DRIVE]);
    put(b, x0 + 20, y0 + 4, "[-C-] HDD", c.items[pal::DRIVE]);
    mdir_core::scr::hline(b, x0, y0 + 11, w, '═', c.items[pal::LINE]);
    putw(b, x0, y0 + 12, "", w, c.items[pal::COUNT]);
    put_hl(b, x0, y0 + 12, " ^9^ File ^2^ Dirs ^11,324^ byte. (^80M^)", c.items[pal::COUNT], c.items[pal::COUNT_HI]);
    // MCD
    let m = c.items[pal::MCD];
    let ms = c.items[pal::MCD_STATUS];
    let y = y0 + 14;
    putw(b, x0, y, "", w, ms);
    put_hl(b, x0 + 3, y, "^M^dirIII ^C^hange ^D^irectory", ms, (ms & 0xf0) | 1);
    fill(b, x0, y + 1, w, 3, ' ', m);
    put(b, x0, y + 1, "C:\\ ──────┬── MDIR", m);
    put(b, x0 + 10, y + 2, "└── GAME", m);
    put(b, x0 + 14, y + 1, "MDIR", pal::bar(m));
    putw(b, x0, y + 4, "", w, ms);
    put_hl(b, x0, y + 4, "^F1^-Help ^F2^-Rescan ^F3^-Drive", ms, (ms & 0xf0) | 1);
    put(b, x0 + 26, y + 3, "[game_]", c.items[pal::SEARCH_MCD]);
}

impl Tui for Mset {
    fn draw(&mut self, b: &mut Buffer) {
        let area = b.area;
        let (w, h) = (area.width, area.height);
        let c = self.cfg.colors.clone();
        fill(b, 0, 0, w, h, '░', 0x17);
        let a = c.items[pal::DIALOG];
        let hi = hi_of(a);
        preview(b, &c, 1, 2);
        if self.hidden {
            // any click brings the menu back, as any key does
            mdir_core::mouse::block(area.x, area.y, w, h, vec![mdir_core::mouse::key(KeyCode::Esc)]);
            return;
        }
        let menus_live = self.ed.is_none();
        // menu bar
        putw(b, 0, 0, "", w, 0x70);
        let mut x = 1;
        let mut xs = vec![];
        for (k, (ko, en)) in MENUS.iter().enumerate() {
            xs.push(x);
            let s = format!(" ^{}^ {} ", k + 1, t(ko, en));
            let (na, ha) = if k == self.m { (a, hi) } else { (0x70, 0x74) };
            let x1 = x;
            x = put_hl(b, x, 0, &s, na, ha);
            if menus_live {
                mdir_core::mouse::zone(x1, 0, x - x1, 1, vec![mdir_core::mouse::key(KeyCode::Char((b'1' + k as u8) as char))], vec![]);
            }
        }
        put(b, w.saturating_sub(18), 0, " M-dir MSET ", 0x70);
        // drop-down
        let its = items(self.m);
        let labels: Vec<String> = its.iter().map(|it| self.label(it)).collect();
        let iw = labels.iter().map(|s| hw(s)).max().unwrap_or(10) + 4;
        let ih = labels.len() as u16 + 2;
        let x0 = 40.min(w.saturating_sub(iw + 1)).max(xs[self.m].min(w.saturating_sub(iw + 1)));
        frame(b, x0, 1, iw, ih, a, false);
        shadow(b, x0, 1, iw, ih);
        for (k, l) in labels.iter().enumerate() {
            let (na, ha) = if k == self.i && self.ed.is_none() { (0x70, 0x74) } else { (a, hi) };
            putw(b, x0 + 1, 2 + k as u16, "", iw - 2, na);
            put_hl(b, x0 + 2, 2 + k as u16, l, na, ha);
            if menus_live {
                mdir_core::mouse::zone(x0 + 1, 2 + k as u16, iw - 2, 1, mdir_core::mouse::choose(k), vec![]);
            }
        }
        // help line
        putw(b, 0, h - 1, "", w, 0x70);
        put(b, 1, h - 1, &fit(&self.help_line(), w as usize - 2), 0x70);
        // editors
        match &self.ed {
            Some(Ed::Ext(sel)) => {
                list_box(b, a, t("확장자지정", "Extensions"), &self.cfg.ext.iter().map(|(e, p)| format!("{:<4} {}", e, p)).collect::<Vec<_>>(), *sel, t("^Space^확장자 ^Enter^프로그램 ^Del^삭제 ^Ins^추가 ^^C^/^^P^", "^Space^ext ^Enter^program ^Del^ ^Ins^ ^^C^/^^P^"));
            }
            Some(Ed::Fkey(sel)) => {
                list_box(b, a, t("펑션키지정", "Function keys"), &fkey_names().iter().map(|k| format!("{:<9} {}", fkey_label(k), self.cfg.fkeys.get(k).cloned().unwrap_or_default())).collect::<Vec<_>>(), *sel, t("^Enter^프로그램지정 ^Del^삭제 ^^C^오려두기 ^^P^붙이기", "^Enter^program ^Del^delete ^^C^copy ^^P^paste"));
            }
            Some(Ed::ExtColor(sel)) => {
                let lines: Vec<String> = self.cfg.colors.ext.iter().map(|(e, a)| format!("{:<4} {:02X} ■■■■", e, a)).collect();
                let (bx, by) = list_box(b, a, t("파일확장자별", "By extension"), &lines, *sel, t("^Space^확장자 ^Enter^색깔 ^Del^삭제 ^Ins^추가", "^Space^ext ^Enter^colour ^Del^ ^Ins^"));
                let vis = (h as usize).saturating_sub(8);
                let top = sel.saturating_sub(vis.saturating_sub(1));
                for (k, (_, at)) in self.cfg.colors.ext.iter().enumerate().skip(top).take(vis) {
                    put(b, bx + 10, by + 1 + (k - top) as u16, "■■■■", (c.items[pal::FILE] & 0xf0) | (at & 15));
                }
            }
            Some(Ed::Pick(what, at, bg_only)) => {
                let title = match what {
                    200 => t("메인화면배경", "Main background").to_string(),
                    201 => t("펑션키 배경", "Fkey background").to_string(),
                    w if *w >= 100 => format!(".{}", self.cfg.colors.ext[w - 100].0),
                    w => t(ITEM_NAMES[*w].0, ITEM_NAMES[*w].1).to_string(),
                };
                mdir_core::mouse::block(area.x, area.y, w, h, vec![]);
                let (bx, by) = mdir_core::scr::center_box(b, 54, 21, a, &title);
                let (fg0, bg0) = ((*at & 15) as usize, (*at >> 4) as usize);
                for bgc in 0..16u8 {
                    for fgc in 0..16u8 {
                        let cell = (bgc << 4) | fgc;
                        let s = if cell == *at { "[]" } else { "Ab" };
                        let (cx, cy) = (bx + 2 + fgc as u16 * 3, by + 2 + bgc as u16);
                        put(b, cx, cy, s, cell);
                        // click picks the colour, double click (or a second click) confirms it
                        let mut ks = mdir_core::mouse::move_bar(bg0, bgc as usize);
                        if !*bg_only {
                            let (l, r) = (KeyCode::Left, KeyCode::Right);
                            ks.extend(if fgc as usize >= fg0 { mdir_core::mouse::times(r, fgc as usize - fg0) } else { mdir_core::mouse::times(l, fg0 - fgc as usize) });
                        }
                        mdir_core::mouse::zone(cx, cy, 2, 1, ks, vec![mdir_core::mouse::key(KeyCode::Enter)]);
                    }
                }
                put_hl(b, bx + 2, by + 19, if *bg_only { t("^↑^^↓^ 배경색   ^Enter^ 선택   ^Esc^ 취소", "^Up^/^Down^ background  ^Enter^  ^Esc^") } else { t("^←^^→^ 글자색  ^↑^^↓^ 배경색  ^Enter^ 선택  ^Esc^ 취소", "^Left^/^Right^ text  ^Up^/^Down^ background  ^Enter^  ^Esc^") }, a, hi);
            }
            None => {}
        }
        if let Some((m, _)) = self.modal.as_mut() {
            m.draw(b, &c);
        }
    }

    fn key(&mut self, kk: KeyEvent) -> Flow {
        if let Some((mut m, p)) = self.modal.take() {
            let r = m.key(kk);
            if let MRes::Keep = r {
                self.modal = Some((m, p));
                return Flow::Cont;
            }
            return self.modal_done(p, r);
        }
        if self.hidden {
            self.hidden = false;
            return Flow::Cont;
        }
        if self.ed.is_some() {
            self.list_key(kk);
            return Flow::Cont;
        }
        let n = items(self.m).len();
        match kk.code {
            KeyCode::Char('x') | KeyCode::Char('X') if alt(&kk) => return self.quit(),
            KeyCode::Esc => return self.quit(),
            KeyCode::Tab => self.hidden = true,
            KeyCode::Left => {
                self.m = (self.m + 5) % 6;
                self.i = 0
            }
            KeyCode::Right => {
                self.m = (self.m + 1) % 6;
                self.i = 0
            }
            KeyCode::Up => self.i = (self.i + n - 1) % n,
            KeyCode::Down => self.i = (self.i + 1) % n,
            KeyCode::Home => self.i = 0,
            KeyCode::End => self.i = n - 1,
            KeyCode::Char(d @ '1'..='6') => {
                self.m = d as usize - '1' as usize;
                self.i = 0;
            }
            KeyCode::Enter => return self.activate(),
            _ => {}
        }
        Flow::Cont
    }
}

/// A scrolling list box on the right; returns its top-left corner.
fn list_box(b: &mut Buffer, a: u8, title: &str, lines: &[String], sel: usize, keys: &str) -> (u16, u16) {
    let area = b.area;
    let w = 60u16.min(area.width - 2);
    let h = area.height.saturating_sub(4);
    let (x, y) = (area.width - w - 1, 2);
    // the list box takes every click: rows select (double click = Enter), elsewhere nothing
    mdir_core::mouse::block(area.x, area.y, area.width, area.height, vec![]);
    frame(b, x, y, w, h, a, false);
    shadow(b, x, y, w, h);
    put_hl(b, x + 2, y, &format!(" {} ", title), a, hi_of(a));
    let vis = (h as usize).saturating_sub(3);
    let top = sel.saturating_sub(vis.saturating_sub(1));
    for (k, l) in lines.iter().enumerate().skip(top).take(vis) {
        let na = if k == sel { 0x70 } else { a };
        putw(b, x + 1, y + 1 + (k - top) as u16, &format!(" {}", l), w - 2, na);
        mdir_core::mouse::zone(x + 1, y + 1 + (k - top) as u16, w - 2, 1, mdir_core::mouse::select(k), vec![mdir_core::mouse::key(KeyCode::Enter)]);
    }
    put_hl(b, x + 2, y + h - 2, keys, a, hi_of(a));
    (x, y)
}

fn self_test(out: &Path) -> bool {
    let dir = std::env::temp_dir().join(format!("mset-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("M.CFG");
    let mut s = Mset { cfg: Config::defaults(path.clone()), m: 0, i: 0, hidden: false, modal: None, ed: None, clip: None, back_ed: None };
    let none = KeyModifiers::NONE;
    let mut r = Report::new();
    s.key(k(KeyCode::Right, none));
    s.key(k(KeyCode::Right, none));
    s.key(k(KeyCode::Down, none));
    r.step("move", s.m == 2 && s.i == 1, format!("Right x2 opened menu {} (메인화면), Down chose item {}", s.m + 1, s.i + 1));
    // toggle "가로선" (HORILINE) and set a function key, then save with Mdir > 저장하기
    let idx = items(2).iter().position(|it| matches!(it, Item::Opt("HORILINE"))).unwrap();
    s.i = idx;
    s.key(k(KeyCode::Enter, none));
    s.key(k(KeyCode::Char('2'), none));
    s.key(k(KeyCode::Down, none));
    s.key(k(KeyCode::Enter, none));
    s.key(k(KeyCode::Enter, none));
    if let Some((Modal::Input { edit, .. }, _)) = s.modal.as_mut() {
        edit.set("vv !.!");
        edit.fresh = false;
    }
    s.key(k(KeyCode::Enter, none));
    s.key(k(KeyCode::Esc, none));
    s.key(k(KeyCode::Char('1'), none));
    s.i = 3;
    s.key(k(KeyCode::Enter, none));
    let back = Config::load_from(&path);
    let saved = !back.flag("HORILINE") && back.fkeys.get("F2").map(|v| v == "vv !.!").unwrap_or(false);
    r.step("act", saved, format!("가로선 off and F2=vv !.! written to M.CFG: {}", saved));
    s.modal = None;
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    s.draw(&mut b);
    let txt = screen_text(&b);
    r.step("hud", (txt.contains("메인화면") || txt.contains("Main")) && txt.contains("Path C:\\DOS"), "menu bar, drop-down, preview of the main screen and the help line are drawn");

    // mouse: click the Program menu, its Extensions item, then an extension row (double click edits it)
    mdir_core::text::set_korean(false);
    use mdir_core::mouse::click;
    let c1 = click(&mut s, "2 Program", false);
    let on_prog = s.m == 1;
    let c2 = click(&mut s, "Extensions", false);
    let in_list = matches!(s.ed, Some(Ed::Ext(0)));
    let row: String = s.cfg.ext.get(2).map(|(e, p)| format!("{:<4} {}", e, p).chars().take(10).collect()).unwrap_or_default();
    let c3 = click(&mut s, &row, false);
    let picked = matches!(s.ed, Some(Ed::Ext(2)));
    let c4 = click(&mut s, &row, true);
    let editing = s.modal.is_some();
    // colour picker: the bottom-left cell is white background, black text (0xF0)
    s.modal = None;
    s.ed = Some(Ed::Pick(pal::FILE, 0x07, false));
    let c5 = click(&mut s, "Ab", false);
    let moved = matches!(s.ed, Some(Ed::Pick(_, 0xf0, _)));
    let c6 = click(&mut s, "[]", true);
    let chosen = s.ed.is_none() && s.cfg.colors.items[pal::FILE] == 0xf0;
    r.step(
        "mouse",
        c1 && on_prog && c2 && in_list && c3 && picked && c4 && editing && c5 && moved && c6 && chosen,
        format!("menu click → Program: {}, item click → extension list: {}, row click → row 3: {}, double click → edit box: {}, colour click → F0: {}, double click chose it: {}", on_prog, in_list, picked, editing, moved, chosen),
    );
    let _ = std::fs::remove_dir_all(&dir);
    let _ = r.save(out);
    r.all_passed()
}

fn main() {
    let cli = term::cli();
    mdir_core::text::init_lang_from_env();
    if let Some(p) = cli.self_test {
        std::process::exit(if self_test(&p) { 0 } else { 1 });
    }
    let cfg = if cli.screenshot.is_some() { Config::defaults(std::env::temp_dir().join("mset-shot.cfg")) } else { Config::load() };
    cfg.apply_language();
    let mut s = Mset { cfg, m: 2, i: 0, hidden: false, modal: None, ed: None, clip: None, back_ed: None };
    if let Some(p) = cli.screenshot {
        if let Err(e) = term::screenshot(&mut s, &p) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }
    if let Err(e) = term::run(&mut s) {
        eprintln!("{}", e);
    }
}
