//! MFF — MdirIII FileFind 3.10: finds files by spec on a drive (or every drive with /A),
//! then lists them with sorting, selection, delete, view, run and "go to the file" (Enter).
use mdir_core::cfg::Config;
use mdir_core::fsx::{self, Attr};
use mdir_core::modal::{alt, ctrl, MRes, Modal};
use mdir_core::pal::{self, Colors};
use mdir_core::ratatui::buffer::Buffer;
use mdir_core::ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mdir_core::ratatui::layout::Rect;
use mdir_core::scr::{center_box, fill, put, put_hl, putw, screen_text};
use mdir_core::term::{self, k, Flow, Report, RunReq, Tui};
use mdir_core::text::{commas, fit, fit_right, fmt_date, split_ext, t, wild};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

struct Hit {
    name: String,
    dir: PathBuf,
    size: u64,
    mtime: SystemTime,
    attr: Attr,
    tag: u32,
}

#[derive(PartialEq)]
enum Ask {
    Stop,
    Delete,
    Info,
}

struct Mff {
    specs: Vec<String>,
    stack: Vec<PathBuf>,
    hits: Vec<Hit>,
    searching: bool,
    scanning: String,
    cur: usize,
    top: usize,
    sort: char,
    compact: bool,
    search: Option<String>,
    modal: Option<(Modal, Ask)>,
    rows: usize,
    tag_seq: u32,
    colors: Colors,
    goto: Option<PathBuf>,
    quit_after_msg: bool,
}

const MAX_HITS: usize = 30000;

fn time8(t: SystemTime) -> String {
    let (_, _, _, h, mi, s) = mdir_core::text::local_fields(t);
    format!("{:>2}:{:02}:{:02}", h, mi, s)
}

impl Mff {
    fn new(roots: Vec<PathBuf>, specs: Vec<String>, colors: Colors) -> Mff {
        let mut stack = roots;
        stack.reverse();
        Mff { specs, stack, hits: vec![], searching: true, scanning: String::new(), cur: 0, top: 0, sort: 'x', compact: false, search: None, modal: None, rows: 20, tag_seq: 0, colors, goto: None, quit_after_msg: false }
    }

    /// Searches for a while (about 40 ms) and returns; called until the stack is empty.
    fn step(&mut self, budget_ms: u128) {
        let start = Instant::now();
        while let Some(d) = self.stack.pop() {
            self.scanning = d.display().to_string();
            if let Ok(rd) = std::fs::read_dir(&d) {
                let mut subs = vec![];
                for e in rd.flatten() {
                    let Ok(ft) = e.file_type() else { continue };
                    let name = e.file_name().to_string_lossy().to_string();
                    if ft.is_dir() {
                        let p = e.path();
                        if !ft.is_symlink() && !(cfg!(unix) && ["/proc", "/sys", "/dev", "/run"].iter().any(|s| p == Path::new(s))) {
                            subs.push(p);
                        }
                    } else if self.specs.iter().any(|s| wild(s, &name)) && self.hits.len() < MAX_HITS {
                        if let Ok(m) = e.metadata() {
                            self.hits.push(Hit { attr: fsx::attr(&name, &m), name, dir: d.clone(), size: m.len(), mtime: m.modified().unwrap_or(SystemTime::UNIX_EPOCH), tag: 0 });
                        }
                    }
                }
                subs.sort();
                subs.reverse();
                self.stack.extend(subs);
            }
            if start.elapsed().as_millis() >= budget_ms {
                return;
            }
        }
        self.finish();
    }

    fn finish(&mut self) {
        self.searching = false;
        self.stack.clear();
        if self.hits.is_empty() {
            self.modal = Some((Modal::msg(t("찾기가 끝났습니다", "Search completed"), &[t("해당하는 파일이 없습니다", "no matching files")]), Ask::Info));
            self.quit_after_msg = true;
        }
        self.resort();
    }

    fn resort(&mut self) {
        let key = |h: &Hit| (split_ext(&h.name).0.to_lowercase(), split_ext(&h.name).1.to_lowercase());
        match self.sort {
            'N' => self.hits.sort_by(|a, b| key(a).cmp(&key(b))),
            'E' => self.hits.sort_by(|a, b| key(a).1.cmp(&key(b).1).then(key(a).0.cmp(&key(b).0))),
            'S' => self.hits.sort_by(|a, b| a.size.cmp(&b.size).then(key(a).cmp(&key(b)))),
            'T' => self.hits.sort_by(|a, b| a.mtime.cmp(&b.mtime)),
            'A' => self.hits.sort_by(|a, b| a.attr.text().cmp(&b.attr.text()).then(key(a).cmp(&key(b)))),
            _ => self.hits.sort_by(|a, b| a.dir.cmp(&b.dir).then(key(a).cmp(&key(b)))),
        }
    }

    fn line(&self, h: &Hit, w: usize) -> String {
        let (b, e) = split_ext(&h.name);
        let name = format!("{} {}", fit(b, 8), fit(e, 3));
        let dir = h.dir.display().to_string();
        let head = if self.compact {
            format!("{} {}  ", name, fit_right(&commas(h.size), 11))
        } else {
            format!("{} {} {} {} {} ", name, fit_right(&commas(h.size), 10), fmt_date(h.mtime), time8(h.mtime), h.attr.text().replace('.', "_"))
        };
        let room = w.saturating_sub(mdir_core::text::width(&head));
        let d = if mdir_core::text::width(&dir) + 1 >= room { fit(&dir, room) } else { format!("{} {}", dir, "-".repeat(room - dir.chars().count() - 1)) };
        format!("{}{}", head, d)
    }

    fn find(&mut self, s: &str, from: usize) -> bool {
        let n = self.hits.len();
        let s = s.to_lowercase();
        for i in 0..n {
            let j = (from + i) % n;
            if self.hits[j].name.to_lowercase().starts_with(&s) {
                self.cur = j;
                return true;
            }
        }
        false
    }

    fn cur_path(&self) -> Option<PathBuf> {
        self.hits.get(self.cur).map(|h| h.dir.join(&h.name))
    }
}

impl Tui for Mff {
    fn draw(&mut self, b: &mut Buffer) {
        let area = b.area;
        let (w, h) = (area.width, area.height);
        let c = &self.colors;
        let bg = c.items[pal::FILE] & 0xf0;
        fill(b, 0, 0, w, h, ' ', bg | 7);
        let st = c.items[pal::FKEY];
        let hi = c.items[pal::FKEY_HI];
        putw(b, 0, 0, "", w, st);
        put_hl(b, 1, 0, &format!("{} ^{}^", t("M-dir 파일찾기 -", "M-dir FileFind -"), self.specs.join(";")), st, hi);
        if let Some(s) = &self.search {
            putw(b, w.saturating_sub(18), 0, &format!("[{}_]", s), 17, c.items[pal::SEARCH]);
        }
        self.rows = (h as usize).saturating_sub(3).max(1);
        if self.searching {
            let (x, y) = center_box(b, 64, 6, c.items[pal::DIALOG], t("파일찾기", "FileFind"));
            let a = c.items[pal::DIALOG];
            put(b, x + 2, y + 2, &fit(&format!("Searching for {}", self.scanning), 60), a);
            put_hl(b, x + 2, y + 3, &format!("^{}^ {}", commas(self.hits.len() as u64), t("파일찾았음", "file found")), a, mdir_core::scr::hi_of(a));
        } else {
            if self.cur < self.top {
                self.top = self.cur;
            }
            if self.cur >= self.top + self.rows {
                self.top = self.cur + 1 - self.rows;
            }
            for (i, hh) in self.hits.iter().enumerate().skip(self.top).take(self.rows) {
                let y = 1 + (i - self.top) as u16;
                let ext = split_ext(&hh.name).1;
                let mut a = c.ext_color(ext).map(|x| bg | (x & 15)).unwrap_or(c.items[pal::FILE]);
                if hh.tag > 0 {
                    a = 0x10 | (a & 0x0f);
                }
                if i == self.cur {
                    a = pal::bar(a);
                }
                put(b, 0, y, &self.line(hh, w as usize), a);
            }
            let py = h - 2;
            let pa = c.items[pal::COUNT];
            putw(b, 0, py, "", w, pa);
            if let Some(p) = self.cur_path() {
                put(b, 0, py, &fit(&p.display().to_string(), w as usize - 16), pa);
            }
            put(b, w - 15, py, &format!("[{:>5}/{:<5}]", self.cur + 1, self.hits.len()), pa);
            let total: u64 = self.hits.iter().map(|x| x.size).sum();
            let tg: Vec<&Hit> = self.hits.iter().filter(|x| x.tag > 0).collect();
            let tb: u64 = tg.iter().map(|x| x.size).sum();
            let s = format!(
                " ^{}^ File ^{}^ Bytes │^{}^│  ^{}^ file ^{}^ Bytes selected",
                commas(self.hits.len() as u64),
                commas(total),
                self.sort,
                commas(tg.len() as u64),
                commas(tb)
            );
            putw(b, 0, h - 1, "", w, st);
            put_hl(b, 0, h - 1, &s, st, hi);
        }
        if let Some((m, _)) = self.modal.as_mut() {
            m.draw(b, c);
        }
    }

    fn key(&mut self, kk: KeyEvent) -> Flow {
        if let Some((mut m, ask)) = self.modal.take() {
            match m.key(kk) {
                MRes::Keep => self.modal = Some((m, ask)),
                MRes::Choice(0) if ask == Ask::Stop => self.finish(),
                MRes::Choice(0) if ask == Ask::Delete => {
                    let mut i = 0;
                    while i < self.hits.len() {
                        if self.hits[i].tag > 0 {
                            let p = self.hits[i].dir.join(&self.hits[i].name);
                            if std::fs::remove_file(&p).is_ok() {
                                self.hits.remove(i);
                                continue;
                            }
                        }
                        i += 1;
                    }
                    self.cur = self.cur.min(self.hits.len().saturating_sub(1));
                    if self.hits.is_empty() {
                        self.modal = Some((Modal::msg(t("프로그램종료", "No more files"), &[t("파일이 모두 삭제되었습니다", "No more files")]), Ask::Info));
                        self.quit_after_msg = true;
                    }
                }
                _ => {
                    if self.quit_after_msg {
                        return Flow::Quit;
                    }
                }
            }
            return Flow::Cont;
        }
        if self.searching {
            if kk.code == KeyCode::Esc {
                self.modal = Some((Modal::yes_no(t("파일찾기", "FileFind"), &[t("찾기를 중단할까요?", "Stop searching?")], true), Ask::Stop));
            }
            return Flow::Cont;
        }
        let n = self.hits.len();
        let last = n.saturating_sub(1);
        let rows = self.rows;
        let (c, a) = (ctrl(&kk), alt(&kk));
        if let Some(s) = self.search.clone() {
            match kk.code {
                KeyCode::Backspace => {
                    let mut s = s;
                    s.pop();
                    self.search = if s.is_empty() { None } else { Some(s) };
                    return Flow::Cont;
                }
                KeyCode::Char('l') if c => {
                    let f = self.cur + 1;
                    self.find(&s, f);
                    return Flow::Cont;
                }
                _ => {}
            }
        }
        match kk.code {
            KeyCode::Esc => {
                if self.search.take().is_none() {
                    return Flow::Quit;
                }
            }
            KeyCode::Char('x') if a => return Flow::Quit,
            KeyCode::F(1) => {
                let lines: Vec<&str> = if mdir_core::text::korean() {
                    vec!["이동 - ^↑^,^↓^,^PgUp^,^PgDn^,^Home^,^End^", "정렬 - ^Alt^+^N^ame,^E^xt,^S^ize,^T^ime,^A^ttr,n^O^", "빨리찾기 - 영문자키  /  ^Ctrl^+^L^ - 다음파일", "한   파일 선택/취소 - ^SPACE^", "모든 파일 선택/취소 - ^Alt-U^", "선택된 파일 반전    - ^회색 *^", "선택한 파일 삭제 - ^Alt-D^    MFF 종료 - ^Alt-X^", "^Enter^ - 그 디렉토리로   ^Alt-V^ - 보기   ^Tab^ - 실행"]
                } else {
                    vec!["Arrow - ^Up^,^Down^,^PgUp^,^PgDn^,^Home^,^End^", "Sort  - ^Alt^+^N^ame,^E^xt,^S^ize,^T^ime,^A^ttr,n^O^", "Speed search - letters  /  ^Ctrl^+^L^ - next", "Select/unselect - ^SPACE^", "Select all - ^Alt-U^   Invert - gray ^*^", "Delete selected - ^Alt-D^   Quit - ^Alt-X^", "^Enter^ - go to directory   ^Alt-V^ - view   ^Tab^ - run"]
                };
                self.modal = Some((Modal::msg("HELP", &lines), Ask::Info));
            }
            KeyCode::Up => self.cur = self.cur.saturating_sub(1),
            KeyCode::Down => self.cur = (self.cur + 1).min(last),
            KeyCode::Right if c => self.compact = !self.compact,
            KeyCode::PageUp | KeyCode::Left => self.cur = self.cur.saturating_sub(rows),
            KeyCode::PageDown | KeyCode::Right => self.cur = (self.cur + rows).min(last),
            KeyCode::Home => self.cur = 0,
            KeyCode::End => self.cur = last,
            KeyCode::Char(' ') => {
                if let Some(h) = self.hits.get_mut(self.cur) {
                    if h.tag > 0 {
                        h.tag = 0;
                    } else {
                        self.tag_seq += 1;
                        h.tag = self.tag_seq;
                    }
                }
                self.cur = (self.cur + 1).min(last);
            }
            KeyCode::Char('*') => {
                for h in self.hits.iter_mut() {
                    h.tag = if h.tag > 0 { 0 } else { 1 };
                }
            }
            KeyCode::Char(ch) if a => match ch.to_ascii_uppercase() {
                s @ ('N' | 'E' | 'S' | 'T' | 'A') => {
                    self.sort = s;
                    self.resort();
                }
                'O' => {
                    self.sort = 'x';
                    self.resort();
                }
                'U' => {
                    let any = self.hits.iter().any(|h| h.tag > 0);
                    for h in self.hits.iter_mut() {
                        h.tag = if any { 0 } else { 1 };
                    }
                }
                'D' => {
                    if self.hits.iter().any(|h| h.tag > 0) {
                        self.modal = Some((Modal::yes_no(t("파일지우기", "DELETE"), &[t("선택된 파일을 모두 지울까요?", "Delete selected all files?")], false), Ask::Delete));
                    }
                }
                'V' => {
                    if let Some(p) = self.cur_path() {
                        let vv = term::exe_dir().join(if cfg!(windows) { "vv.exe" } else { "vv" });
                        return Flow::Run(RunReq { cmd: format!("\"{}\" \"{}\"", vv.display(), p.display()), cwd: p.parent().unwrap().to_path_buf(), pause: false, quit_after: false });
                    }
                }
                'X' => return Flow::Quit,
                _ => {}
            },
            KeyCode::Tab => {
                if let Some(p) = self.cur_path() {
                    return Flow::Run(RunReq { cmd: format!("\"{}\"", p.display()), cwd: p.parent().unwrap().to_path_buf(), pause: true, quit_after: false });
                }
            }
            KeyCode::Enter => {
                if let Some(p) = self.cur_path() {
                    self.goto = Some(p);
                    return Flow::Quit;
                }
            }
            KeyCode::Char(ch) if !c && !a => {
                let mut s = self.search.clone().unwrap_or_default();
                s.push(ch);
                self.find(&s, 0);
                self.search = Some(s);
            }
            _ => {}
        }
        Flow::Cont
    }

    fn tick(&mut self) -> Flow {
        if self.searching && self.modal.is_none() {
            self.step(40);
        }
        Flow::Cont
    }
}

/// MFF [drive] [spec[;spec...]] [/A /J /K /?]
fn parse(args: &[String]) -> (Vec<PathBuf>, Vec<String>, bool) {
    let mut roots = vec![];
    let mut specs = vec![];
    let mut all = false;
    for a in args {
        if a.starts_with('/') && !Path::new(a).is_dir() {
            all |= a.to_ascii_lowercase().contains('a');
            continue;
        }
        for part in a.split(';').filter(|s| !s.is_empty()) {
            let p = Path::new(part);
            let drive = part.len() == 2 && part.ends_with(':');
            if drive {
                let l = part.chars().next().unwrap().to_ascii_uppercase();
                if let Some(d) = fsx::drives().into_iter().find(|d| d.letter == l) {
                    roots.push(d.root);
                }
            } else if p.is_dir() {
                roots.push(p.to_path_buf());
            } else {
                specs.push(mdir_core::text::expand_spec(part));
            }
        }
    }
    if all {
        roots = fsx::drives().into_iter().filter(|d| d.kind == "HDD").map(|d| d.root).collect();
    }
    if roots.is_empty() {
        let cwd = std::env::current_dir().unwrap_or_default();
        roots.push(fsx::drive_of(&cwd, &fsx::drives()).map(|d| d.root).unwrap_or(cwd));
    }
    if specs.is_empty() {
        specs.push("*.*".into());
    }
    (roots, specs, all)
}

fn self_test(out: &Path) -> bool {
    let root = std::env::temp_dir().join(format!("mff-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["A", "A/B", "C"] {
        let _ = std::fs::create_dir_all(root.join(d));
    }
    for f in ["A/ONE.DOC", "A/B/TWO.DOC", "C/THREE.TXT", "C/FOUR.DOC"] {
        let _ = std::fs::write(root.join(f), b"mdir");
    }
    let mut r = Report::new();
    let mut m = Mff::new(vec![root.clone()], vec!["*.doc".into()], Colors::default());
    while m.searching {
        m.step(1000);
    }
    let found = m.hits.len();
    let none = KeyModifiers::NONE;
    m.key(k(KeyCode::Down, none));
    m.key(k(KeyCode::Down, none));
    r.step("move", m.cur == 2, format!("found {} *.doc files; Down x2 moved the bar to {}", found, m.cur + 1));
    m.key(k(KeyCode::Char('t'), none));
    let at = m.hits.get(m.cur).map(|h| h.name.clone()).unwrap_or_default();
    m.key(k(KeyCode::Esc, none));
    m.key(k(KeyCode::Char(' '), none));
    let tagged = m.hits.iter().filter(|h| h.tag > 0).count();
    m.key(k(KeyCode::Up, none));
    let f = m.key(k(KeyCode::Enter, none));
    let goto = m.goto.clone().map(|p| p.display().to_string()).unwrap_or_default();
    r.step("act", found == 3 && at == "TWO.DOC" && tagged == 1 && matches!(f, Flow::Quit) && goto.ends_with("TWO.DOC"), format!("speed search 't' found {}, Space tagged {}, Enter chose {}", at, tagged, goto));
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    m.draw(&mut b);
    let s = screen_text(&b);
    r.step("hud", s.contains("File") && s.contains("selected") && s.contains("/3"), "file list, path line with [n/total] and the File/Bytes selected line are drawn");
    let _ = std::fs::remove_dir_all(&root);
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
    if cli.rest.iter().any(|a| a == "/?") {
        println!("{}", t(
            "MFF (M-dir) - 파일 찾기\n사용법: mff [/옵션] [파일스펙[;파일스펙...]]\n옵션: /A  모든 드라이브 찾기 (플로피 제외)\n      /K  한글 메시지   /?  이 도움말",
            "MFF (M-dir) - find files\nUsage: mff [/option] [filespec[;filespec...]]\nOptions: /A  search every drive (not floppies)\n         /K  Korean messages   /?  this help"));
        return;
    }
    if let Some(p) = cli.screenshot {
        // a small sample tree so the picture shows a finished search
        let root = std::env::temp_dir().join(format!("mff-shot-{}", std::process::id()));
        for (d, f, n) in [("QEMM", "QPAT3.EXE", 1077490usize), ("WINDOWS/SYSTEM", "TAESRR.TTF", 1145033), ("HWP30", "HWP386.EXE", 1197813), ("EXCEL", "EXCEL.EXE", 4364544), ("IYAGI/DOWN", "N16E20B3.EXE", 2059405), ("UTIL/HANGUL", "HANJA48.PFT", 1407744)] {
            let _ = std::fs::create_dir_all(root.join(d));
            let _ = std::fs::write(root.join(d).join(f), vec![0u8; n]);
        }
        let mut m = Mff::new(vec![root.clone()], vec!["*.*".into()], cfg.colors.clone());
        while m.searching {
            m.step(1000);
        }
        m.sort = 'S';
        m.resort();
        let r = term::screenshot(&mut m, &p);
        let _ = std::fs::remove_dir_all(&root);
        if let Err(e) = r {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }
    let (roots, specs, _) = parse(&cli.rest);
    let mut m = Mff::new(roots, specs, cfg.colors.clone());
    if let Err(e) = term::run(&mut m) {
        eprintln!("{}", e);
    }
    if let Some(p) = m.goto {
        // tell M (Alt-I) where to put its bar; otherwise just say where the file is
        match std::env::var("MDIR_GOTO") {
            Ok(f) => {
                let _ = std::fs::write(f, p.display().to_string());
            }
            Err(_) => println!("{} {}", t("이동:", "Go to"), p.display()),
        }
    }
}
