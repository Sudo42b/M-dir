//! DSCMAN 1.0 — cleans MdirIII/4DOS description files over a whole drive: removes descriptions of
//! files that no longer exist and merges repeated ones (DSCMAN.EXE messages).
use mdir_core::fsx::{Descs, DESC_FILE};
use mdir_core::ratatui::buffer::Buffer;
use mdir_core::ratatui::layout::Rect;
use mdir_core::scr::{fill, put};
use mdir_core::term::{self, Report};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Default, Debug)]
struct Stats {
    ok: usize,
    failed: usize,
    deleted: usize,
}

/// Cleans one DESCRIPT.ION; returns how many descriptions were dropped.
fn clean_dir(dir: &Path) -> std::io::Result<usize> {
    let raw = std::fs::read(dir.join(DESC_FILE))?;
    let lines = mdir_core::text::decode_as(&raw, mdir_core::text::detect(&raw)).lines().filter(|l| !l.trim().is_empty()).count();
    let mut d = Descs::load(dir);
    let names: HashSet<String> = std::fs::read_dir(dir)?.flatten().map(|e| e.file_name().to_string_lossy().to_lowercase()).collect();
    d.map.retain(|k, _| names.contains(k));
    let dropped = lines.saturating_sub(d.map.len());
    if dropped > 0 {
        d.save(dir)?;
    }
    Ok(dropped)
}

fn walk(dir: &Path, st: &mut Stats) {
    if dir.join(DESC_FILE).is_file() {
        match clean_dir(dir) {
            Ok(n) => {
                st.ok += 1;
                st.deleted += n;
            }
            Err(_) => st.failed += 1,
        }
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.file_type().map(|t| t.is_dir() && !t.is_symlink()).unwrap_or(false) {
                let p = e.path();
                if cfg!(unix) && ["/proc", "/sys", "/dev", "/run"].iter().any(|s| p == Path::new(s)) {
                    continue;
                }
                walk(&p, st);
            }
        }
    }
}

fn usage() -> Vec<String> {
    if mdir_core::text::korean() {
        vec!["DSCMAN (M-dir) - 주석 파일(DESCRIPT.ION) 정리".into(), "드라이브 전체의 주석 파일을 찾아, 없는 파일의 주석과".into(), "겹친 주석을 지웁니다.".into(), "".into(), "사용법: DSCMAN 드라이브명:".into(), "사용예: DSCMAN c:   <- c:드라이브 청소".into(), "        DSCMAN a:   <- a:드라이브 청소".into()]
    } else {
        vec!["DSCMAN (M-dir) - cleans DESCRIPT.ION description files".into(), "".into(), "Usage: DSCMAN drive:".into(), "Example: DSCMAN c:".into(), "         DSCMAN a:".into()]
    }
}

fn summary(label: &str, st: &Stats) -> Vec<String> {
    if mdir_core::text::korean() {
        vec![format!("{}:드라이브 정리 완료", label), format!(" {} 디렉토리의 정리성공, {} 디렉토리의 정리실패", st.ok, st.failed), format!(" {} 개의 불필요한 주석 삭제시킴", st.deleted)]
    } else {
        vec![format!("{}: drive {} directory successfully completed, {} directory Error occured.", label, st.ok, st.failed), format!("{} description deleted.", st.deleted)]
    }
}

/// "c:" → that drive's root; a folder path → itself.
fn root_of(arg: &str) -> Option<(String, PathBuf)> {
    let p = Path::new(arg);
    if arg.len() == 2 && arg.ends_with(':') {
        let l = arg.chars().next()?.to_ascii_uppercase();
        let d = mdir_core::fsx::drives().into_iter().find(|d| d.letter == l)?;
        return Some((l.to_string(), d.root));
    }
    p.is_dir().then(|| (arg.trim_end_matches(['/', '\\']).to_string(), p.to_path_buf()))
}

fn self_test(out: &Path) -> bool {
    let dir = std::env::temp_dir().join(format!("dscman-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(dir.join("SUB"));
    let _ = std::fs::write(dir.join("KEEP.TXT"), b"x");
    let _ = std::fs::write(dir.join("SUB").join("ONE.DOC"), b"x");
    let _ = std::fs::write(dir.join(DESC_FILE), "keep.txt kept\r\ngone.txt removed file\r\nkeep.txt kept twice\r\n");
    let _ = std::fs::write(dir.join("SUB").join(DESC_FILE), "one.doc first\r\nold.doc gone\r\n");
    let mut st = Stats::default();
    walk(&dir, &mut st);
    let top = Descs::load(&dir);
    let sub = Descs::load(&dir.join("SUB"));
    let mut r = Report::new();
    r.step("move", st.ok == 2, format!("walked the tree and found {} description files", st.ok));
    let ok = st.deleted == 3 && top.get("KEEP.TXT") == Some("kept twice") && top.get("gone.txt").is_none() && sub.get("one.doc") == Some("first") && sub.map.len() == 1;
    r.step("act", ok, format!("{} descriptions removed (missing files and the repeated one); kept: {:?} / {:?}", st.deleted, top.map, sub.map));
    let lines = summary("T", &st);
    r.step("hud", lines.len() >= 2, lines.join(" / "));
    let _ = std::fs::remove_dir_all(&dir);
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
    if let Some(p) = cli.screenshot {
        // the console as it looks after a run
        let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
        fill(&mut b, 0, 0, 80, 25, ' ', 0x07);
        put(&mut b, 0, 0, "C:\\>DSCMAN C:", 0x07);
        let mut y = 1;
        for l in usage().iter().take(3).chain(std::iter::once(&String::new())).chain(summary("C", &Stats { ok: 12, failed: 0, deleted: 5 }).iter()) {
            put(&mut b, 0, y, l, 0x07);
            y += 1;
        }
        put(&mut b, 0, y + 1, "C:\\>_", 0x07);
        if let Err(e) = mdir_core::shot::save_png(&b, &p) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }
    let Some((label, root)) = cli.rest.iter().find_map(|a| root_of(a)) else {
        for l in usage() {
            println!("{}", l);
        }
        return;
    };
    let mut st = Stats::default();
    walk(&root, &mut st);
    for l in summary(&label, &st) {
        println!("{}", l);
    }
}
