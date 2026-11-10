//! MCOPY — MdirIII diskCOPY 3.10, for disk image files: read the source once into memory, then
//! write it to the target as many times as asked (MCOPY.DOC menu, INFO box and messages).
use mdir_core::cfg::Config;
use mdir_core::modal::{alt, MRes, Modal};
use mdir_core::pal::Colors;
use mdir_core::ratatui::buffer::Buffer;
use mdir_core::ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mdir_core::ratatui::layout::Rect;
use mdir_core::scr::screen_text;
use mdir_core::term::{self, k, Flow, Report, Tui};
use mdir_core::text::{commas, t};
use mdir_core::toolui::{self, Tool};
use std::path::{Path, PathBuf};

const MAX_IMAGE: u64 = 64 << 20;

#[derive(Clone, Copy, PartialEq)]
enum What {
    Info,
    Source,
    Target,
    Count,
}

struct Mcopy {
    src: String,
    dst: String,
    copies: u32,
    format: bool,
    data: Option<Vec<u8>>,
    sel: usize,
    modal: Option<(Modal, What)>,
    progress: Option<String>,
    colors: Colors,
}

const MENU_N: usize = 11;

/// Track/head counters for the progress box, from a floppy size.
fn geometry(len: usize) -> (usize, usize) {
    let spt = match len / 1024 {
        360 | 720 => 9,
        1200 => 15,
        2880 => 36,
        _ => 18,
    };
    let tracks = (len / 512).div_ceil(spt * 2).max(1);
    (tracks, 2)
}

impl Mcopy {
    fn menu(&self) -> Vec<String> {
        if mdir_core::text::korean() {
            vec![" 디스크복사       ^Ins^ ".into(), "-".into(), " 읽기               ^R^ ".into(), " 쓰기               ^W^ ".into(), "-".into(), " 원본드라이브      ^F3^ ".into(), " 대상드라이브      ^F4^ ".into(), " 복사매수          ^F5^ ".into(), "-".into(), " 포맷여부           ^F^ ".into(), " 종료              ^@X^ ".into()]
        } else {
            vec![" DiskCopy         ^Ins^ ".into(), "-".into(), " Read               ^R^ ".into(), " Write              ^W^ ".into(), "-".into(), " Source            ^F3^ ".into(), " Target            ^F4^ ".into(), " Number Of Copy    ^F5^ ".into(), "-".into(), " Format             ^F^ ".into(), " Quit              ^@X^ ".into()]
        }
    }

    fn info(&self) -> Vec<String> {
        let onoff = |b: bool| if b { "ON" } else { "OFF" };
        if mdir_core::text::korean() {
            vec![format!(" 원본 드라이브  -  ^{}^", self.src), format!(" 대상 드라이브  -  ^{}^", self.dst), "-".into(), format!(" 쓰기가능       - ^{}^", onoff(self.data.is_some())), format!(" 포맷여부       - ^{}^", onoff(self.format)), "-".into(), format!(" 복사매수       - ^{}^", self.copies)]
        } else {
            vec![format!(" SOURCE Drive   -  ^{}^", self.src), format!(" TARGET Drive   -  ^{}^", self.dst), "-".into(), format!(" Write          - ^{}^", onoff(self.data.is_some())), format!(" Format         - ^{}^", onoff(self.format)), "-".into(), format!(" Number of Copy - ^{}^", self.copies)]
        }
    }

    fn read(&mut self) -> Result<String, String> {
        let p = Path::new(&self.src);
        let m = std::fs::metadata(p).map_err(|_| format!("{} {}", t("준비되지 않았습니다:", "Not ready error in drive"), self.src))?;
        if m.len() > MAX_IMAGE {
            return Err(t("대상드라이브로 복사할 수 없습니다", "Cannot diskcopy to target drive").into());
        }
        let d = std::fs::read(p).map_err(|e| e.to_string())?;
        let (tracks, heads) = geometry(d.len());
        self.progress = Some(format!("{} Track : ^{}^  Head : ^{}^", t("읽기", "Read "), tracks - 1, heads - 1));
        let n = d.len();
        self.data = Some(d);
        Ok(format!("{} {} bytes", t("읽었습니다:", "Read:"), commas(n as u64)))
    }

    /// Target file for copy `i` (1-based): the target itself, or NAME-2.IMG, NAME-3.IMG ... for more copies.
    fn target(&self, i: u32) -> PathBuf {
        let p = PathBuf::from(&self.dst);
        if i <= 1 {
            return p;
        }
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ext = p.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
        p.with_file_name(format!("{}-{}{}", stem, i, ext))
    }

    fn write(&mut self) -> Result<String, String> {
        let Some(d) = self.data.clone() else { return Err(t("먼저 읽기를 하세요", "Read a disk first").into()) };
        let (tracks, heads) = geometry(d.len());
        for i in 1..=self.copies {
            let p = self.target(i);
            if let Some(dir) = p.parent().filter(|x| !x.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            if p.exists() && std::fs::metadata(&p).map(|m| m.permissions().readonly()).unwrap_or(false) {
                return Err(format!("{} {}", t("쓰기방지탭이 있습니다:", "Write prodected in drive"), p.display()));
            }
            std::fs::write(&p, &d).map_err(|_| format!("{} {}", t("쓰기방지탭이 있습니다:", "Write prodected in drive"), p.display()))?;
        }
        self.progress = Some(format!("{} Track : ^{}^  Head : ^{}^  {} ^{}^", t("쓰기", "Write"), tracks - 1, heads - 1, t("매수:", "Copy:"), self.copies));
        Ok(format!("{} {} {}", self.copies, t("개의 복사본을 만들었습니다", "copies written"), self.target(1).display()))
    }

    fn report(&mut self, r: Result<String, String>) {
        let (title, msg) = match r {
            Ok(s) => (t("안내", "Info"), s),
            Err(e) => (t("에러", "Error"), e),
        };
        self.modal = Some((Modal::msg(title, &[&msg]), What::Info));
    }

    fn act(&mut self, i: usize) -> Flow {
        match i {
            0 => {
                let r = self.read().and_then(|_| self.write());
                self.report(r);
            }
            2 => {
                let r = self.read();
                self.report(r);
            }
            3 => {
                let r = self.write();
                self.report(r);
            }
            5 => self.modal = Some((Modal::input(t("원본드라이브", "Source"), &[t("읽어올 디스크 이미지 파일", "Disk image to read")], &self.src), What::Source)),
            6 => self.modal = Some((Modal::input(t("대상드라이브", "Target"), &[t("기록할 디스크 이미지 파일", "Disk image to write")], &self.dst), What::Target)),
            7 => self.modal = Some((Modal::input(t("복사매수", "Number Of Copy"), &["1..9999"], &self.copies.to_string()), What::Count)),
            9 => self.format = !self.format,
            10 => return Flow::Quit,
            _ => {}
        }
        Flow::Cont
    }
}

impl Tui for Mcopy {
    fn draw(&mut self, b: &mut Buffer) {
        let menu = self.menu();
        let info = self.info();
        let title = t("^M^-dir 디스크복사 (이미지 파일)", "^M^-dir DiskCopy (image files)");
        let status = t(" ^Ins^-디스크복사  ^R^-읽기  ^W^-쓰기  ^F3^-원본 ^F4^-대상 ^F5^-매수   ^ESC^ - 종료하기", " ^Ins^-DiskCopy  ^R^-Read  ^W^-Write  ^F3^-Source ^F4^-Target ^F5^-Copies   ^ESC^ - Exit Dos");
        toolui::draw(b, &self.colors, &Tool { title, menu: &menu, sel: self.sel, info: &info, status, progress: self.progress.as_deref() });
        if let Some((m, _)) = self.modal.as_mut() {
            m.draw(b, &self.colors);
        }
    }

    fn key(&mut self, kk: KeyEvent) -> Flow {
        if let Some((mut m, w)) = self.modal.take() {
            match (m.key(kk), w) {
                (MRes::Keep, _) => self.modal = Some((m, w)),
                (MRes::Text(s), What::Source) if !s.trim().is_empty() => {
                    self.src = s.trim().into();
                    self.data = None;
                }
                (MRes::Text(s), What::Target) if !s.trim().is_empty() => self.dst = s.trim().into(),
                (MRes::Text(s), What::Count) => {
                    if let Ok(n) = s.trim().parse::<u32>() {
                        self.copies = n.clamp(1, 9999);
                    }
                }
                _ => {}
            }
            return Flow::Cont;
        }
        let a = alt(&kk);
        match kk.code {
            KeyCode::Up => {
                self.sel = (self.sel + MENU_N - 1) % MENU_N;
                if [1, 4, 8].contains(&self.sel) {
                    self.sel -= 1;
                }
            }
            KeyCode::Down => {
                self.sel = (self.sel + 1) % MENU_N;
                if [1, 4, 8].contains(&self.sel) {
                    self.sel += 1;
                }
            }
            KeyCode::Enter => return self.act(self.sel),
            KeyCode::Insert => return self.act(0),
            KeyCode::Char('r') | KeyCode::Char('R') => return self.act(2),
            KeyCode::Char('w') | KeyCode::Char('W') => return self.act(3),
            KeyCode::F(3) => return self.act(5),
            KeyCode::F(4) => return self.act(6),
            KeyCode::F(5) => return self.act(7),
            KeyCode::Char('f') | KeyCode::Char('F') if !a => return self.act(9),
            KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('x') | KeyCode::Char('X') if a => return Flow::Quit,
            _ => {}
        }
        Flow::Cont
    }
}

fn new(src: &str, dst: &str, copies: u32, colors: Colors) -> Mcopy {
    Mcopy { src: src.into(), dst: dst.into(), copies, format: false, data: None, sel: 0, modal: None, progress: None, colors }
}

fn self_test(out: &Path) -> bool {
    let dir = std::env::temp_dir().join(format!("mcopy-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join("SRC.IMG");
    let image: Vec<u8> = (0..737_280u32).map(|i| (i % 251) as u8).collect();
    let _ = std::fs::write(&src, &image);
    let dst = dir.join("DST.IMG");
    let mut m = new(&src.display().to_string(), &dst.display().to_string(), 2, Colors::default());
    let none = KeyModifiers::NONE;
    let mut r = Report::new();
    m.key(k(KeyCode::Down, none));
    m.key(k(KeyCode::Down, none));
    r.step("move", m.sel == 3, format!("Down x2 moved the menu bar to item {} (Write)", m.sel + 1));
    m.key(k(KeyCode::Insert, none));
    let a = std::fs::read(&dst).unwrap_or_default() == image;
    let b2 = std::fs::read(dir.join("DST-2.IMG")).unwrap_or_default() == image;
    r.step("act", a && b2, format!("Ins read SRC.IMG once and wrote 2 identical copies: {} {}", a, b2));
    m.modal = None;
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    m.draw(&mut b);
    let s = screen_text(&b);
    r.step("hud", s.contains("INFO") && (s.contains("복사매수") || s.contains("Number of Copy")) && s.contains("Track"), "menu, INFO box (source, target, write, format, copies) and the track/head progress are drawn");

    // mouse: a command click runs it, a status-line hint click presses its key
    use mdir_core::mouse::click;
    mdir_core::text::set_korean(false);
    let c1 = click(&mut m, "Source  ", false);
    let src_box = matches!(&m.modal, Some((Modal::Input { .. }, _)));
    m.modal = None;
    let c2 = click(&mut m, "F4-Target", false);
    let dst_box = matches!(&m.modal, Some((Modal::Input { .. }, _)));
    r.step("mouse", c1 && src_box && c2 && dst_box, format!("Source click opened the source box: {}, F4 hint opened the target box: {}", src_box, dst_box));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = r.save(out);
    r.all_passed()
}

fn main() {
    let cli = term::cli();
    mdir_core::text::init_lang_from_env();
    let cfg = Config::load();
    cfg.apply_language();
    if let Some(p) = cli.self_test {
        std::process::exit(if self_test(&p) { 0 } else { 1 });
    }
    // MCOPY [source] [target] [copies]: a number may come anywhere, like MCOPY 10 a:
    let mut paths = vec![];
    let mut copies = None;
    for a in &cli.rest {
        match a.parse::<u32>() {
            Ok(n) => copies = Some(n.clamp(1, 9999)),
            Err(_) => paths.push(a.clone()),
        }
    }
    let src = paths.first().cloned().unwrap_or_else(|| "A.IMG".into());
    let dst = paths.get(1).cloned().unwrap_or_else(|| src.clone());
    let mut m = new(&src, &dst, copies.unwrap_or(1), cfg.colors.clone());
    if let Some(p) = cli.screenshot {
        m.progress = Some("Read  Track : ^79^  Head : ^1^".into());
        if let Err(e) = term::screenshot(&mut m, &p) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }
    if !paths.is_empty() || copies.is_some() {
        // command-line form: copy at once and say what happened
        match m.read().and_then(|_| m.write()) {
            Ok(s) => println!("{}", s),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        return;
    }
    if let Err(e) = term::run(&mut m) {
        eprintln!("{}", e);
    }
}
