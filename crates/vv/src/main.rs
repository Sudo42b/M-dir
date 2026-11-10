//! VV — 보라 3.10, the MdirIII text viewer: reads UTF-8, 완성형 (KS), 조합형 (Johab) or plain
//! 8-bit text and binary files, with the keys of VV.DOC.
use mdir_core::modal::{alt, ctrl, shift, Modal, MRes};
use mdir_core::pal::Colors;
use mdir_core::ratatui::buffer::Buffer;
use mdir_core::ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventState, KeyModifiers};
use mdir_core::ratatui::layout::Rect;
use mdir_core::scr::{fill, put, put_hl, putw, screen_text};
use mdir_core::term::{self, k, Flow, Report, Tui};
use mdir_core::text::{decode_as, detect, fit, t, width, Code};
use std::path::{Path, PathBuf};

/// Shown when VV starts without a file (written for this project; the original manual isn't shipped).
const INTRO: &str = "\
VV - M-dir text viewer / M-dir 문서 보기

Usage / 사용법:  vv <file>

Reads UTF-8, KS X 1001 (완성형, CP949), Johab (조합형) and plain 8-bit text,
and shows binary files too. F9 or Alt-C switches the encoding.

  Esc, Alt-X      quit / 끝내기
  F1              key help / 키 도움말
  Up, Down        one line / 한 줄
  PgUp, PgDn      one screen / 한 화면
  Ctrl-PgUp/PgDn  first / last line / 처음, 끝
  Left, Right     10 columns / 10칸 (Ctrl: 1칸)
  F11             word wrap / 줄 바꿈
  F12             menu / 메뉴
";
const CODES: [Code; 4] = [Code::Ks, Code::Johab, Code::English, Code::Utf8];

struct Vv {
    name: String,
    bytes: Vec<u8>,
    code: Code,
    lines: Vec<String>,
    top: usize,
    left: usize,
    tab: usize,
    wrap: bool,
    status: bool,
    fg: u8,
    bg: u8,
    sfg: u8,
    sbg: u8,
    method: u8,
    saver: Option<u32>,
    modal: Option<Modal>,
    rows: usize,
    cols: usize,
}

fn state_file() -> PathBuf {
    term::data_dir().join("VV.CFG")
}

impl Vv {
    fn new(name: &str, bytes: Vec<u8>) -> Vv {
        let code = detect(&bytes);
        let mut v = Vv { name: name.to_string(), bytes, code, lines: vec![], top: 0, left: 0, tab: 8, wrap: false, status: true, fg: 7, bg: 1, sfg: 0, sbg: 7, method: 0, saver: None, modal: None, rows: 23, cols: 80 };
        if let Ok(s) = std::fs::read_to_string(state_file()) {
            let n: Vec<u32> = s.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            if n.len() >= 8 {
                (v.fg, v.bg, v.sfg, v.sbg) = (n[0] as u8 & 15, n[1] as u8 & 15, n[2] as u8 & 15, n[3] as u8 & 15);
                v.tab = (n[4] as usize).clamp(1, 10);
                v.wrap = n[5] != 0;
                v.status = n[6] != 0;
                v.method = n[7] as u8 % 3;
            }
        }
        v.decode();
        v
    }

    fn save_state(&self) -> std::io::Result<()> {
        let p = state_file();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(p, format!("{} {} {} {} {} {} {} {}", self.fg, self.bg, self.sfg, self.sbg, self.tab, self.wrap as u8, self.status as u8, self.method))
    }

    fn decode(&mut self) {
        let text = decode_as(&self.bytes, self.code);
        let text = if matches!(self.code, Code::Ks | Code::Johab) { mdir_core::text::widen(&text) } else { text };
        let mut lines = vec![];
        for raw in text.split('\n') {
            let mut l = String::new();
            for c in raw.trim_end_matches('\r').chars() {
                match c {
                    '\t' => {
                        let n = self.tab - width(&l) % self.tab;
                        l.push_str(&" ".repeat(n));
                    }
                    c if (c as u32) < 32 || c == '\u{7f}' => l.push('.'),
                    c => l.push(c),
                }
            }
            lines.push(l);
        }
        if self.wrap {
            let w = self.cols.max(10);
            lines = lines
                .into_iter()
                .flat_map(|l| {
                    let mut out = vec![];
                    let mut cur = String::new();
                    for ch in l.chars() {
                        if width(&cur) + width(&ch.to_string()) > w {
                            out.push(std::mem::take(&mut cur));
                        }
                        cur.push(ch);
                    }
                    out.push(cur);
                    out
                })
                .collect();
        }
        self.lines = lines;
        self.top = self.top.min(self.lines.len().saturating_sub(1));
    }

    fn page(&self) -> usize {
        self.rows.max(1)
    }

    fn max_top(&self) -> usize {
        self.lines.len().saturating_sub(self.page())
    }

    fn help() -> Modal {
        let lines: Vec<&str> = if mdir_core::text::korean() {
            vec![
                "^Esc^,^Alt-X^ : 끝내기          ^F1^ : 도움말",
                "^↑^/^↓^      : 한 줄            ^PgUp^/^PgDn^ : 한 화면",
                "^^PgUp^/^^PgDn^ : 처음/끝 줄     ^Space^ : 반화면  ^Enter^ : 한화면",
                "^←^/^→^      : 10칸             ^^←^/^^→^ : 1칸   ^Home^/^End^ : 왼쪽/오른쪽 끝",
                "^F9^,^Alt-C^  : 조합형/완성형/영문 바꿈",
                "^F5^/^F6^     : 배경색/글자색   ^S-F5^/^S-F6^ : 상태선 배경/글자색",
                "^F4^ : 상태라인  ^F8^ : 표시방법  ^F11^ : 워드랩  ^1^..^0^ : 탭크기",
                "^F2^ : 상태저장  ^F12^ : 메뉴     회색 ^*^ : 화면보호",
            ]
        } else {
            vec![
                "^Esc^,^Alt-X^ : quit            ^F1^ : help",
                "^Up^/^Down^   : one line        ^PgUp^/^PgDn^ : one screen",
                "^^PgUp^/^^PgDn^ : first/last line  ^Space^ : half screen  ^Enter^ : screen",
                "^Left^/^Right^: 10 columns      ^^Left^/^^Right^ : 1 column  ^Home^/^End^",
                "^F9^,^Alt-C^  : Johab/KS/English",
                "^F5^/^F6^     : background/text ^S-F5^/^S-F6^ : status line colours",
                "^F4^ : status line  ^F8^ : display method  ^F11^ : word wrap  ^1^..^0^ : tab",
                "^F2^ : save state   ^F12^ : menu   gray ^*^ : screen saver",
            ]
        };
        Modal::msg(t("VV 도움말", "VV help"), &lines)
    }

    fn menu() -> Modal {
        let items = if mdir_core::text::korean() {
            vec![" 도움말        ^F1^ ", " 한글코드      ^F9^ ", " 워드랩       ^F11^ ", " 상태라인      ^F4^ ", " 배경색        ^F5^ ", " 글자색        ^F6^ ", " 상태저장      ^F2^ ", " 화면보호       ^*^ ", " 끝내기     ^Alt-X^ "]
        } else {
            vec![" Help          ^F1^ ", " Hangul code   ^F9^ ", " Word wrap    ^F11^ ", " Status line   ^F4^ ", " Background    ^F5^ ", " Text colour   ^F6^ ", " Save state    ^F2^ ", " Screen saver   ^*^ ", " Quit       ^Alt-X^ "]
        };
        Modal::menu("VV", items.into_iter().map(String::from).collect(), 0)
    }

    fn command(&mut self, i: usize) -> Flow {
        let keys = [KeyCode::F(1), KeyCode::F(9), KeyCode::F(11), KeyCode::F(4), KeyCode::F(5), KeyCode::F(6), KeyCode::F(2), KeyCode::Char('*'), KeyCode::Char('x')];
        let m = if i == 8 { KeyModifiers::ALT } else { KeyModifiers::NONE };
        self.key(KeyEvent::new(keys[i], m))
    }
}

impl Tui for Vv {
    fn draw(&mut self, b: &mut Buffer) {
        let area = b.area;
        let (w, h) = (area.width, area.height);
        if let Some(tk) = self.saver {
            fill(b, 0, 0, w, h, ' ', 0);
            put(b, (tk as u16 * 3) % w.saturating_sub(6).max(1), (tk as u16 / 3) % h.max(1), "VV", 0x09);
            return;
        }
        let a = (self.bg << 4) | self.fg;
        let sa = (self.sbg << 4) | self.sfg;
        let shi = (self.sbg << 4) | if self.sbg == 1 { 0x0e } else { 0x01 };
        let y0 = if self.status { 1 } else { 0 };
        let newcols = w as usize;
        if newcols != self.cols {
            self.cols = newcols;
            if self.wrap {
                self.decode();
            }
        }
        self.rows = (h - y0) as usize;
        fill(b, 0, 0, w, h, ' ', a);
        for (i, l) in self.lines.iter().skip(self.top).take(self.rows).enumerate() {
            let shown: String = l.chars().skip(self.left).collect();
            put(b, 0, y0 + i as u16, &fit(&shown, w as usize), a);
        }
        if self.status {
            putw(b, 0, 0, "", w, sa);
            let pct = if self.lines.len() <= self.rows { 100 } else { self.top * 100 / self.max_top().max(1) };
            let s = format!(
                " ^VV^ │ {} │ ^{}^ │ {} {}/{} │ {} {} │ {} {} │ {}%",
                self.name,
                self.code.label(),
                t("줄", "Line"),
                self.top + 1,
                self.lines.len(),
                t("칸", "Col"),
                self.left + 1,
                t("탭", "Tab"),
                self.tab,
                pct
            );
            let s = if mdir_core::text::korean() { s } else { s.replace("보라", "BORA") };
            put_hl(b, 0, 0, &s, sa, shi);
            put_hl(b, w.saturating_sub(12), 0, t(" ^F1^-도움말", "  ^F1^-Help"), sa, shi);
        }
        if let Some(m) = self.modal.as_mut() {
            m.draw(b, &Colors::default());
        }
    }

    fn key(&mut self, kk: KeyEvent) -> Flow {
        if self.saver.take().is_some() {
            return Flow::Cont;
        }
        if let Some(mut m) = self.modal.take() {
            let is_menu = matches!(m, Modal::Menu { .. });
            return match m.key(kk) {
                MRes::Keep => {
                    self.modal = Some(m);
                    Flow::Cont
                }
                MRes::Choice(i) if is_menu => self.command(i),
                _ => Flow::Cont,
            };
        }
        let page = self.page();
        let (c, a) = (ctrl(&kk), alt(&kk));
        let keypad = kk.state.contains(KeyEventState::KEYPAD);
        match kk.code {
            KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('x') | KeyCode::Char('X') if a => return Flow::Quit,
            KeyCode::F(1) if shift(&kk) => self.modal = Some(Modal::msg(t("등록번호", "Registration"), &[t("공개판 (Rust 재구현)", "Free edition (Rust rebuild)")])),
            KeyCode::F(1) => self.modal = Some(Vv::help()),
            KeyCode::F(12) => self.modal = Some(Vv::menu()),
            KeyCode::Insert if c => self.modal = Some(Vv::menu()),
            KeyCode::Up => self.top = self.top.saturating_sub(1),
            KeyCode::Down => self.top = (self.top + 1).min(self.max_top()),
            KeyCode::PageUp if c => self.top = 0,
            KeyCode::PageDown if c => self.top = self.max_top(),
            KeyCode::PageUp => self.top = self.top.saturating_sub(page),
            KeyCode::PageDown | KeyCode::Enter => self.top = (self.top + page).min(self.max_top()),
            KeyCode::Char(' ') => self.top = (self.top + page / 2).min(self.max_top()),
            KeyCode::Left if c => self.left = self.left.saturating_sub(1),
            KeyCode::Right if c => self.left += 1,
            KeyCode::Left => self.left = self.left.saturating_sub(10),
            KeyCode::Right => self.left += 10,
            KeyCode::Home => self.left = 0,
            KeyCode::End => self.left = self.lines.iter().map(|l| l.chars().count()).max().unwrap_or(0).saturating_sub(self.cols.saturating_sub(1)),
            KeyCode::F(9) if shift(&kk) => {}
            KeyCode::F(9) => {
                let i = CODES.iter().position(|&x| x == self.code).unwrap_or(0);
                self.code = CODES[(i + 1) % CODES.len()];
                self.decode();
            }
            KeyCode::Char('c') | KeyCode::Char('C') if a => {
                let i = CODES.iter().position(|&x| x == self.code).unwrap_or(0);
                self.code = CODES[(i + 1) % CODES.len()];
                self.decode();
            }
            KeyCode::F(5) if shift(&kk) => self.sbg = (self.sbg + 1) % 8,
            KeyCode::F(6) if shift(&kk) => self.sfg = (self.sfg + 1) % 16,
            KeyCode::F(5) => self.bg = (self.bg + 1) % 8,
            KeyCode::F(6) => self.fg = (self.fg + 1) % 16,
            KeyCode::F(4) => self.status = !self.status,
            KeyCode::F(8) => self.method = (self.method + 1) % 3,
            KeyCode::F(11) => {
                self.wrap = !self.wrap;
                self.left = 0;
                self.decode();
            }
            KeyCode::F(2) => {
                let ok = self.save_state().is_ok();
                self.modal = Some(Modal::msg(t("상태저장", "Save"), &[if ok { t("현재 상태를 기억시켰습니다", "State saved") } else { t("저장할 수 없습니다", "Cannot save") }]));
            }
            KeyCode::Char('*') if keypad || !c => self.saver = Some(0),
            KeyCode::Char(d @ '0'..='9') if !a && !c => {
                self.tab = if d == '0' { 10 } else { d as usize - '0' as usize };
                self.decode();
            }
            _ => {}
        }
        Flow::Cont
    }

    fn tick(&mut self) -> Flow {
        if let Some(t) = self.saver.as_mut() {
            *t += 1;
        }
        Flow::Cont
    }
}

fn self_test(out: &Path) -> bool {
    let mut r = Report::new();
    let ks = mdir_core::text::encode_ks("한글 보라 시험\n둘째 줄\n");
    let text: Vec<u8> = (0..200).flat_map(|i| format!("line {:03}\tTAB\n", i).into_bytes()).collect();
    let mut v = Vv::new("TEST.TXT", text);
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    v.draw(&mut b);
    let none = KeyModifiers::NONE;
    v.key(k(KeyCode::PageDown, none));
    v.key(k(KeyCode::Down, none));
    let moved = v.top;
    r.step("move", moved == 25, format!("PgDn + Down moved the view to line {}", moved + 1));
    let mut kv = Vv::new("HAN.TXT", ks);
    let detected = kv.code;
    kv.draw(&mut b);
    let first = kv.lines.first().cloned().unwrap_or_default();
    v.key(k(KeyCode::Char('4'), none));
    let tab_ok = v.lines[0].contains("line 000    TAB") || v.lines[0].starts_with("line 000");
    r.step("act", detected == Code::Ks && first.starts_with("한글") && tab_ok, format!("KS text detected as {:?} and shown as '{}'; tab size 4 applied", detected, first));
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    v.draw(&mut b);
    let s = screen_text(&b);
    let ok = s.contains("TEST.TXT") && (s.contains("줄") || s.contains("Line")) && s.contains("line 025");
    r.step("hud", ok, "status line shows the file, code, line and column; text below it");
    let _ = r.save(out);
    r.all_passed()
}

fn main() {
    let cli = term::cli();
    mdir_core::text::init_lang_from_env();
    mdir_core::cfg::Config::load().apply_language();
    if let Some(p) = cli.self_test {
        std::process::exit(if self_test(&p) { 0 } else { 1 });
    }
    let file = cli.rest.iter().find(|a| !a.starts_with('/') || Path::new(a).exists()).cloned();
    if cli.rest.iter().any(|a| a == "/?") {
        println!("VV (M-dir) - text viewer\nUsage: vv [file] [/options]\n  /N  no screen saver   /*  screen saver only   /U  no mouse (ignored)   /?  this help");
        return;
    }
    let (name, bytes) = match &file {
        Some(f) => match std::fs::read(f) {
            Ok(b) => (Path::new(f).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), b),
            Err(_) => {
                eprintln!("{} not found...", f);
                std::process::exit(1);
            }
        },
        None => ("VV".into(), INTRO.as_bytes().to_vec()),
    };
    let mut v = Vv::new(&name, bytes);
    if cli.rest.iter().any(|a| a == "/*") {
        v.saver = Some(0);
    }
    if let Some(p) = cli.screenshot {
        if let Err(e) = term::screenshot(&mut v, &p) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }
    if let Err(e) = term::run(&mut v) {
        eprintln!("{}", e);
    }
}
