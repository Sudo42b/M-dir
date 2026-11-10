//! M — the MdirIII 3.10 main file manager, rebuilt for terminals.
mod act;
mod app;
mod arc;
mod draw;
mod help;
mod list;
mod mcd;
mod ops;
mod selftest;

use app::App;
use mdir_core::cfg::Config;
use mdir_core::term;
use mdir_core::text::t;
use std::path::{Path, PathBuf};

const USAGE_KO: &str = "M-dir - MdirIII 3.10 호환 터미널 파일 관리자 (MIT OR Apache-2.0)
MdirIII 및 MDIR COMPUTER와 관계없는 독립 재구현입니다.

사용법: m [/옵션...] [파일스펙] [디렉토리]
예:     m /L *.exe /2      소문자로, 2열로 *.exe만 보기
        m dow *.com        dow로 시작하는 디렉토리로 가서 *.com 보기
옵션:
 1..6  열 개수                  N E S T B Q  정렬(이름 확장자 크기 시간 색 실행)
 =     줄 수 자동 맞춤 끄기     K J !        완성형 / 조합형 / 자동 인식
 +     키 반복 빠르게           R            디렉토리 트리 다시 읽기
 A     속성 보기                c            MCD로 시작 (C: MCD 후 끝내기)
 Z     숨김 파일 포함           D            디렉토리와 실행 파일만
 L     소문자                   V            28줄 (v: 43/50줄)
 X     (호환용, 효과 없음)      P            DIR 모드 (p: 디렉토리만 바꿈)";

const USAGE_EN: &str = "M-dir - a terminal file manager compatible with MdirIII 3.10 (MIT OR Apache-2.0)
An independent reimplementation, not affiliated with MdirIII or MDIR COMPUTER.

Usage:   m [/options...] [filespec] [directory]
Example: m /L *.exe /2      lower-case names, 2 columns, only *.exe
         m dow *.com        go to the directory starting with dow, show *.com
Options:
 1..6  columns                  N E S T B Q  sort (name ext size time colour exec)
 =     don't fit lines          K J !        KS / Johab / auto-detect Korean
 +     fast key repeat          R            rescan the directory tree
 A     show attributes          c            start in MCD (C: quit after MCD)
 Z     include hidden files     D            directories and executables only
 L     lower case               V            28 lines (v: 43/50)
 X     (compatibility, no effect) P          DIR mode (p: change directory only)";

#[derive(Default)]
struct Opts {
    flags: Vec<char>,
    specs: Vec<String>,
    dir: Option<String>,
}

fn parse(args: &[String]) -> Opts {
    let mut o = Opts::default();
    for a in args {
        let is_opt = (a.starts_with('/') && !Path::new(a).exists()) || (a.starts_with('-') && a.len() > 1);
        if is_opt {
            o.flags.extend(a.chars().filter(|c| *c != '/' && *c != '-'));
            if a == "-" {
                o.flags.push('-');
            }
        } else if a.contains('*') || a.contains('?') || a.contains(';') || (a.starts_with('.') && a.len() > 1 && !a.contains(['/', '\\']) && !Path::new(a).is_dir()) {
            o.specs.extend(a.split(';').filter(|s| !s.is_empty()).map(|s| s.to_string()));
        } else {
            o.dir = Some(a.clone());
        }
    }
    o
}

/// Finds a directory by exact name, then by its first letters, under the current directory.
fn find_dir(name: &str) -> Option<PathBuf> {
    let p = PathBuf::from(name);
    if p.is_dir() {
        return Some(p);
    }
    let low = name.to_lowercase();
    let mut subs: Vec<PathBuf> = std::fs::read_dir(".").ok()?.flatten().filter(|e| e.path().is_dir()).map(|e| e.path()).collect();
    subs.sort();
    subs.into_iter().find(|p| p.file_name().map(|f| f.to_string_lossy().to_lowercase().starts_with(&low)).unwrap_or(false))
}

fn apply_flags(app: &mut App, flags: &[char]) {
    for &f in flags {
        match f {
            '1'..='6' => app.cols_mode = f as u8 - b'0',
            'N' | 'n' => app.sort = list::Sort::Name,
            'E' | 'e' => app.sort = list::Sort::Ext,
            'S' | 's' => app.sort = list::Sort::Size,
            'T' | 't' => app.sort = list::Sort::Time,
            'B' | 'b' => app.sort = list::Sort::Color,
            'Q' | 'q' => app.sort = list::Sort::Exec,
            'J' | 'j' | 'K' | 'k' | '!' => mdir_core::text::set_korean(true),
            'D' | 'd' => app.exe_only = true,
            'L' | 'l' => app.lower = true,
            'A' | 'a' => {
                app.show_attr = true;
                app.hidden = true
            }
            'Z' | 'z' => app.hidden = true,
            'V' => app.lines = Some(28),
            'v' => app.lines = Some(50),
            '=' => app.auto_line = false,
            _ => {}
        }
    }
    app.reload();
}

fn dir_mode(app: &mut App) {
    let fmt = app.fmt();
    let lines: Vec<String> = app.entries.iter().filter(|e| e.kind != list::Kind::Drive).map(|e| list::entry_text(e, 38, &fmt, app.descs.get(&e.name), None)).collect();
    println!(" {} {}", t("경로", "Path"), app.dir.display());
    for pair in lines.chunks(2) {
        println!(" {}", pair.join(" │ "));
    }
    let files = app.entries.iter().filter(|e| e.is_file()).count();
    let dirs = app.entries.iter().filter(|e| e.kind == list::Kind::Dir).count();
    let bytes: u64 = app.entries.iter().filter(|e| e.is_file()).map(|e| e.size).sum();
    println!("{}", list::count_line(files, dirs, bytes, app.space).replace('^', ""));
}

/// A directory laid out like the sample screen in M.DOC, for the screenshot.
fn demo_dir() -> PathBuf {
    let root = std::env::temp_dir().join(format!("mdir-demo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let at = |y, mo, d, h, mi| mdir_core::text::local_time(y, mo, d, h, mi, 0);
    let dirs = [("DOS", at(1993, 12, 11, 20, 41)), ("FAX", at(1993, 12, 12, 18, 30)), ("GRP", at(1993, 12, 11, 22, 18)), ("HBR", at(1993, 12, 11, 21, 33)), ("HWP21", at(1993, 12, 18, 18, 6)), ("IYG", at(1993, 12, 11, 22, 30)), ("NU", at(1993, 12, 11, 20, 42))];
    let files = [("AUTOEXEC.BAT", 353usize, at(1994, 1, 4, 13, 44)), ("COMMAND.COM", 52925, at(1993, 3, 10, 6, 0)), ("MDIRTREE.DIR", 475, at(1994, 1, 7, 0, 40)), ("CONFIG.SYS", 296, at(1994, 1, 4, 13, 44)), ("README.DOC", 1706, at(1998, 12, 21, 3, 10)), ("M.EXE", 88875, at(1998, 12, 21, 3, 10)), ("VV.EXE", 65968, at(1998, 12, 21, 3, 10)), ("PKZIP.EXE", 42166, at(1993, 2, 1, 2, 4)), ("HANEE.ZIP", 120344, at(1995, 12, 29, 20, 18)), ("SETUP.HWP", 9216, at(1995, 6, 9, 2, 8))];
    for (d, tm) in dirs {
        let p = root.join(d);
        let _ = std::fs::create_dir_all(&p);
        let _ = std::fs::File::open(&p).and_then(|f| f.set_modified(tm));
    }
    for (f, n, tm) in files {
        let p = root.join(f);
        let _ = std::fs::write(&p, vec![b' '; n]);
        let _ = std::fs::File::options().write(true).open(&p).and_then(|f| f.set_modified(tm));
    }
    root
}

fn main() {
    let cli = term::cli();
    mdir_core::text::init_lang_from_env();
    if let Some(shot) = cli.screenshot {
        let dir = demo_dir();
        std::env::set_var("M_CFG", dir.join("M.CFG"));
        let cfg = Config::defaults(std::env::temp_dir().join("mdir-shot.cfg"));
        let mut app = App::new(cfg, dir.clone());
        app.cols_mode = 2;
        app.cur = app.entries.iter().position(|e| e.name == "COMMAND.COM").unwrap_or(0);
        app.space = Some((566_231_040, 881_088));
        // MDIR_SHOT_KEYS opens another screen first (menu, MCD, help...), for checking those screens
        if let Ok(keys) = std::env::var("MDIR_SHOT_KEYS") {
            use mdir_core::ratatui::crossterm::event::{KeyCode as C, KeyModifiers as M};
            for kname in keys.split(',') {
                let ev = match kname {
                    "F1" => term::k(C::F(1), M::NONE),
                    "F10" => term::k(C::F(10), M::NONE),
                    "F11" => term::k(C::F(11), M::NONE),
                    "F12" => term::k(C::F(12), M::NONE),
                    "Right" => term::k(C::Right, M::NONE),
                    "Down" => term::k(C::Down, M::NONE),
                    "Space" => term::k(C::Char(' '), M::NONE),
                    "AltC" => term::k(C::Char('c'), M::ALT),
                    "Alt1" => term::k(C::Char('1'), M::ALT),
                    "Alt4" => term::k(C::Char('4'), M::ALT),
                    "Enter" => term::k(C::Enter, M::NONE),
                    "Slash" => term::k(C::Char('/'), M::NONE),
                    "CtrlZ" => term::k(C::Char('z'), M::CONTROL),
                    _ => continue,
                };
                app.key(ev);
            }
        }
        if let Err(e) = term::screenshot(&mut app, &shot) {
            eprintln!("screenshot: {}", e);
            std::process::exit(1);
        }
        let _ = std::fs::remove_dir_all(dir);
        return;
    }
    if let Some(p) = cli.self_test {
        let ok = selftest::run(&p);
        std::process::exit(if ok { 0 } else { 1 });
    }
    let o = parse(&cli.rest);
    if o.flags.contains(&'?') {
        println!("{}", if mdir_core::text::korean() { USAGE_KO } else { USAGE_EN });
        return;
    }
    if o.flags.contains(&'M') || o.flags.contains(&'m') {
        for row in 0..16u8 {
            for col in 0..16u8 {
                let a = row * 16 + col;
                let (r, g, b) = mdir_core::pal::VGA[(a & 15) as usize];
                let (br, bgc, bb) = mdir_core::pal::VGA[(a >> 4) as usize];
                print!("\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m {:02X} ", r, g, b, br, bgc, bb, a);
            }
            println!("\x1b[0m");
        }
        return;
    }
    let cfg = Config::load();
    let start = o.dir.as_deref().and_then(find_dir).unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let mut app = App::new(cfg, start);
    app.filter = o.specs.clone();
    apply_flags(&mut app, &o.flags);
    if o.flags.contains(&'P') || o.flags.contains(&'p') {
        dir_mode(&mut app);
        return;
    }
    if o.flags.contains(&'R') || o.flags.contains(&'r') {
        let m = mcd::Mcd::open(&app, mcd::Purpose::Go);
        let mut m = m;
        m.rescan(app.cfg.flag("MCDSORT"));
    }
    if o.flags.contains(&'c') || o.flags.contains(&'C') || app.cfg.flag("MCDSTART") {
        app.quit_mcd = o.flags.contains(&'C') || app.cfg.flag("MCDQUIT");
        app.open_mcd(mcd::Purpose::Go);
    }
    if let Some(d) = &o.dir {
        if find_dir(d).is_none() {
            // not found here: let MCD look for it by name
            app.open_mcd(mcd::Purpose::Go);
            if let app::Mode::Mcd(m) = &mut app.mode {
                m.find(d, false);
                m.search = Some(d.clone());
            }
        }
    }
    if let Err(e) = term::run(&mut app) {
        eprintln!("{}", e);
    }
    if let Ok(f) = std::env::var("MDIR_CWD_FILE") {
        let _ = std::fs::write(f, app.dir.to_string_lossy().as_bytes());
    }
}
