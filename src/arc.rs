//! Archive view (Enter on an archive), extraction, creation and deletion inside archives.
//! ZIP (and ZIP self-extractors) are read and written directly; other kinds use their own
//! programs with the command lines MdirIII used.
use crate::act::Act;
use crate::app::App;
use crate::list::{Entry, Kind};
use mdir_core::fsx::Attr;
use mdir_core::modal::Modal;
use mdir_core::term::Flow;
use mdir_core::text::{local_time, split_ext, t, wild};
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const KINDS: [&str; 15] = ["LZH", "ICE", "ZIP", "ARJ", "ARC", "OMP", "ZOO", "SQZ", "RAR", "UC2", "HPK", "ACE", "LHA", "JAR", "7Z"];

/// (extract, add, move) programs per kind, after M.EXE's table (LHA x /a, ARJ x -a, ...).
fn tools(kind: &str) -> (&'static str, &'static str, &'static str) {
    match kind {
        "LZH" | "ICE" | "LHA" => ("lha x", "lha a", "lha m"),
        "ARJ" => ("arj x -a", "arj a -a", "arj m -a"),
        "ARC" => ("pkxarc -x", "pkarc a", "pkarc m -a"),
        "OMP" => ("tomp x -a", "tomp a -a", "tomp m"),
        "ZOO" => ("zoo x/", "zoo -add", "zoo -move"),
        "SQZ" => ("sqz x", "sqz a", "sqz a"),
        "RAR" => ("unrar x", "rar a", "rar m"),
        "UC2" => ("uc e -s", "uc a", "uc a"),
        "HPK" => ("hpack x -a", "hpack a -a", "hpack a -f -a"),
        "ACE" => ("ace x", "ace a", "ace m"),
        "7Z" => ("7z x", "7z a", "7z a -sdel"),
        _ => ("unzip -o", "zip -r", "zip -rm"),
    }
}

pub fn kind_of(p: &Path) -> Option<String> {
    let ext = split_ext(&p.file_name()?.to_string_lossy()).1.to_ascii_uppercase();
    let mut head = [0u8; 8];
    let n = std::fs::File::open(p).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
    if n >= 4 && &head[..4] == b"PK\x03\x04" {
        return Some("ZIP".into());
    }
    if KINDS.contains(&ext.as_str()) {
        return Some(if ext == "JAR" { "ZIP".into() } else { ext });
    }
    // self-extracting ZIP: the end-of-central-directory record is near the end
    if n >= 2 && &head[..2] == b"MZ" && zip_ok(p) {
        return Some("ZIP".into());
    }
    None
}

fn zip_ok(p: &Path) -> bool {
    std::fs::File::open(p).ok().and_then(|f| zip::ZipArchive::new(f).ok()).is_some()
}

pub fn is_archive(p: &Path) -> bool {
    p.is_file() && kind_of(p).is_some()
}

pub struct Item {
    pub path: String,
    pub size: u64,
    pub mtime: SystemTime,
    pub dir: bool,
}

pub struct ArcView {
    pub path: PathBuf,
    pub kind: String,
    pub items: Vec<Item>,
    pub cwd: String,
}

impl ArcView {
    pub fn open(p: &Path) -> Result<ArcView, String> {
        let kind = kind_of(p).ok_or("not an archive")?;
        if kind != "ZIP" {
            return Err(t("압축파일 보기 실패", "Can not view").into());
        }
        let f = std::fs::File::open(p).map_err(|e| e.to_string())?;
        let mut z = zip::ZipArchive::new(f).map_err(|_| t("파괴된 파일입니다", "Broken file").to_string())?;
        let mut items = vec![];
        for i in 0..z.len() {
            let Ok(e) = z.by_index(i) else { continue };
            let mtime = e.last_modified().map(|d| local_time(d.year() as i32, d.month() as u32, d.day() as u32, d.hour() as u32, d.minute() as u32, d.second() as u32)).unwrap_or(SystemTime::UNIX_EPOCH);
            items.push(Item { path: e.name().trim_end_matches('/').to_string(), size: e.size(), mtime, dir: e.is_dir() });
        }
        Ok(ArcView { path: p.to_path_buf(), kind, items, cwd: String::new() })
    }

    fn entry(name: &str, inner: &str, kind: Kind, size: u64, mtime: SystemTime) -> Entry {
        Entry { name: name.to_string(), kind, size, mtime, attr: Attr::default(), exec: false, tag: 0, drive: None, inner: inner.to_string() }
    }

    /// List lines: "<EXIT>" (folders shown) or "<Exit>" (all files flat), then folders and files.
    pub fn entries(&self, emul: bool) -> Vec<Entry> {
        let mut v = vec![ArcView::entry(if emul { "<EXIT>" } else { "<Exit>" }, "", Kind::Exit, 0, SystemTime::UNIX_EPOCH)];
        if !emul {
            for it in self.items.iter().filter(|i| !i.dir) {
                let base = it.path.rsplit('/').next().unwrap_or(&it.path);
                let mut e = ArcView::entry(base, &it.path, Kind::File, it.size, it.mtime);
                e.exec = mdir_core::fsx::EXEC_EXT.iter().any(|x| x.eq_ignore_ascii_case(split_ext(base).1));
                v.push(e);
            }
            return v;
        }
        if !self.cwd.is_empty() {
            v.push(ArcView::entry("..", "", Kind::Up, 0, SystemTime::UNIX_EPOCH));
        }
        let pre = if self.cwd.is_empty() { String::new() } else { format!("{}/", self.cwd) };
        let mut dirs: Vec<String> = vec![];
        for it in &self.items {
            let Some(rest) = it.path.strip_prefix(&pre) else { continue };
            if rest.is_empty() {
                continue;
            }
            match rest.split_once('/') {
                Some((d, _)) => {
                    if !dirs.contains(&d.to_string()) {
                        dirs.push(d.to_string());
                    }
                }
                None if it.dir => {
                    if !dirs.contains(&rest.to_string()) {
                        dirs.push(rest.to_string());
                    }
                }
                None => {
                    let mut e = ArcView::entry(rest, &it.path, Kind::File, it.size, it.mtime);
                    e.exec = mdir_core::fsx::EXEC_EXT.iter().any(|x| x.eq_ignore_ascii_case(split_ext(rest).1));
                    v.push(e);
                }
            }
        }
        for d in dirs {
            let full = format!("{}{}", pre, d);
            v.push(ArcView::entry(&d, &full, Kind::Dir, 0, SystemTime::UNIX_EPOCH));
        }
        v
    }
}

/// Writes the chosen entries (all matching `spec` when `inner` is empty) under `dest`.
pub fn unzip(arc: &Path, dest: &Path, inner: &[String], spec: &str) -> Result<usize, String> {
    let f = std::fs::File::open(arc).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|_| t("파괴된 파일입니다", "Broken file").to_string())?;
    let mut n = 0;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let name = e.name().trim_end_matches('/').to_string();
        let base = name.rsplit('/').next().unwrap_or(&name).to_string();
        let wanted = if inner.is_empty() { e.is_dir() || wild(spec, &base) } else { inner.iter().any(|x| name == *x || name.starts_with(&format!("{}/", x))) };
        if !wanted {
            continue;
        }
        // entries that would leave the folder are refused
        let Some(rel) = e.enclosed_name() else { continue };
        let out = dest.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut w = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut e, &mut w).map_err(|e| e.to_string())?;
        if let Some(d) = e.last_modified() {
            let _ = w.set_modified(local_time(d.year() as i32, d.month() as u32, d.day() as u32, d.hour() as u32, d.minute() as u32, d.second() as u32));
        }
        n += 1;
    }
    Ok(n)
}

fn zip_add<W: std::io::Write + Seek>(z: &mut zip::ZipWriter<W>, base: &Path, rel: &str) -> std::io::Result<()> {
    let p = base.join(rel);
    let m = std::fs::metadata(&p)?;
    let (y, mo, d, h, mi, s) = mdir_core::text::local_fields(m.modified().unwrap_or(SystemTime::UNIX_EPOCH));
    let dt = zip::DateTime::from_date_and_time(y.clamp(1980, 2107) as u16, mo as u8, d as u8, h as u8, mi as u8, s as u8).unwrap_or_default();
    let opt = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).last_modified_time(dt);
    if m.is_dir() {
        z.add_directory(format!("{}/", rel), opt)?;
        for e in std::fs::read_dir(&p)?.flatten() {
            zip_add(z, base, &format!("{}/{}", rel, e.file_name().to_string_lossy()))?;
        }
    } else {
        z.start_file(rel, opt)?;
        std::io::copy(&mut std::fs::File::open(&p)?, z)?;
    }
    Ok(())
}

/// Creates (or adds to) a ZIP file.
pub fn zip_files(arc: &Path, dir: &Path, items: &[String]) -> std::io::Result<()> {
    let mut z = if arc.exists() {
        let f = std::fs::OpenOptions::new().read(true).write(true).open(arc)?;
        zip::ZipWriter::new_append(f).map_err(std::io::Error::other)?
    } else {
        zip::ZipWriter::new(std::fs::File::create(arc)?)
    };
    for it in items {
        zip_add(&mut z, dir, it)?;
    }
    z.finish().map_err(std::io::Error::other)?;
    Ok(())
}

/// Rewrites a ZIP without the given entries (and anything under them).
pub fn zip_delete(arc: &Path, inner: &[String]) -> Result<(), String> {
    let tmp = arc.with_extension("$$$");
    {
        let mut z = zip::ZipArchive::new(std::fs::File::open(arc).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mut w = zip::ZipWriter::new(std::fs::File::create(&tmp).map_err(|e| e.to_string())?);
        for i in 0..z.len() {
            let e = z.by_index_raw(i).map_err(|e| e.to_string())?;
            let name = e.name().trim_end_matches('/').to_string();
            if inner.iter().any(|x| name == *x || name.starts_with(&format!("{}/", x))) {
                continue;
            }
            w.raw_copy_file(e).map_err(|e| e.to_string())?;
        }
        w.finish().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, arc).map_err(|e| e.to_string())
}

impl App {
    pub fn open_archive(&mut self, name: &str) {
        match ArcView::open(&self.dir.join(name)) {
            Ok(v) => {
                self.arc = Some(v);
                self.cur = 0;
                self.search = None;
                self.reload();
            }
            Err(e) => self.error(&e),
        }
    }

    pub fn close_archive(&mut self) {
        let name = self.arc.take().and_then(|a| a.path.file_name().map(|s| s.to_string_lossy().to_string()));
        self.reload();
        if let Some(n) = name {
            if let Some(i) = self.entries.iter().position(|e| e.name == n) {
                self.cur = i;
            }
        }
    }

    fn arc_up(&mut self, to_root: bool) {
        let Some(av) = self.arc.as_mut() else { return };
        if av.cwd.is_empty() || !self.cfg.flag("DIREMUL") {
            self.close_archive();
            return;
        }
        let old = av.cwd.clone();
        av.cwd = if to_root { String::new() } else { old.rsplit_once('/').map(|(a, _)| a.to_string()).unwrap_or_default() };
        self.cur = 0;
        self.reload();
        let last = old.rsplit('/').next().unwrap_or(&old).to_string();
        if let Some(i) = self.entries.iter().position(|e| e.name == last) {
            self.cur = i;
        }
    }

    /// Extracts one entry to a scratch folder and returns its path there.
    fn arc_temp(&mut self, e: &Entry) -> Option<PathBuf> {
        let av = self.arc.as_ref()?;
        let tmp = std::env::temp_dir().join(format!("mdir-(M)-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        match unzip(&av.path, &tmp, std::slice::from_ref(&e.inner), "*.*") {
            Ok(_) => Some(tmp.join(&e.inner)),
            Err(er) => {
                self.error(&er);
                None
            }
        }
    }

    pub fn arc_enter(&mut self, e: &Entry, tab: bool) -> Flow {
        match e.kind {
            Kind::Exit => self.close_archive(),
            Kind::Up => self.arc_up(false),
            Kind::Dir => {
                if let Some(av) = self.arc.as_mut() {
                    av.cwd = e.inner.clone();
                    self.cur = 0;
                    self.reload();
                }
            }
            Kind::File => {
                let ext = e.ext().to_string();
                let def = self.cfg.ext_prog(&ext).map(|s| s.to_string());
                if def.is_none() && !e.exec {
                    return Flow::Cont;
                }
                let Some(p) = self.arc_temp(e) else { return Flow::Cont };
                let dir = p.parent().unwrap_or(Path::new(".")).to_path_buf();
                let file = p.file_name().unwrap().to_string_lossy().to_string();
                let cmd = match def {
                    Some(d) => d.replace("!\\!.!", &p.to_string_lossy()).replace("!.!", &file).replace("%F", &file),
                    None => if cfg!(windows) { file } else { format!("./{}", file) },
                };
                let _ = tab;
                return self.launch(cmd.replace("%%", ""), dir, cmd.contains("%%"), false);
            }
            Kind::Drive => {}
        }
        Flow::Cont
    }

    /// Archive-view meaning of a command; None = same as on the main screen.
    pub fn arc_act(&mut self, a: &Act) -> Option<Flow> {
        let tagged: Vec<String> = {
            let mut v: Vec<&Entry> = self.entries.iter().filter(|e| e.tag > 0).collect();
            if v.is_empty() {
                v = self.cur_entry().filter(|e| matches!(e.kind, Kind::File | Kind::Dir)).into_iter().collect();
            }
            v.into_iter().map(|e| e.inner.clone()).collect()
        };
        Some(match a {
            Act::Parent => {
                self.arc_up(false);
                Flow::Cont
            }
            Act::Root => {
                self.arc_up(true);
                Flow::Cont
            }
            Act::DirEmul => {
                let on = !self.cfg.flag("DIREMUL");
                self.cfg.set_flag("DIREMUL", on);
                if let Some(av) = self.arc.as_mut() {
                    av.cwd.clear();
                }
                self.cur = 0;
                self.reload();
                Flow::Cont
            }
            Act::ArcExtract | Act::Copy | Act::CopyIns => {
                if !tagged.is_empty() {
                    let arcp = self.arc.as_ref().unwrap().path.clone();
                    let init = self.work.clone().unwrap_or(self.dir.clone()).to_string_lossy().to_string();
                    let m = Modal::input(t("압축풀기", "^EXTRACT^"), &[t("압축을 풀어 놓을 곳은?", "^EXTRACT^ file to")], &init);
                    self.modal = Some((m, crate::app::Pending::ExtractDest { archives: vec![arcp], inner: tagged }));
                }
                Flow::Cont
            }
            Act::Delete | Act::DeleteDel => {
                if !tagged.is_empty() {
                    let arcp = self.arc.as_ref().unwrap().path.clone();
                    match zip_delete(&arcp, &tagged) {
                        Ok(_) => {
                            let cwd = self.arc.as_ref().unwrap().cwd.clone();
                            if let Ok(mut v) = ArcView::open(&arcp) {
                                v.cwd = cwd;
                                self.arc = Some(v);
                            }
                            self.reload();
                        }
                        Err(e) => self.error(&e),
                    }
                }
                Flow::Cont
            }
            Act::View => {
                let e = self.cur_entry().cloned()?;
                if e.kind != Kind::File {
                    return Some(Flow::Cont);
                }
                let p = self.arc_temp(&e)?;
                let view = self.cfg.get("VIEW").replace("!\\!.!", "!.!");
                let mdir = mdir_core::term::exe_dir();
                let cmd = view.replace("%M", &format!("{}{}", mdir.display(), std::path::MAIN_SEPARATOR)).replace("%m", &format!("{}{}", mdir.display(), std::path::MAIN_SEPARATOR)).replace("!.!", &format!("\"{}\"", p.display()));
                self.launch(cmd, self.dir.clone(), false, false)
            }
            Act::Run | Act::RunSwap => {
                let e = self.cur_entry().cloned()?;
                self.arc_enter(&e, true)
            }
            Act::Move | Act::Rename | Act::Mkdir | Act::Attrib | Act::Describe | Act::ArcAdd | Act::ArcMove | Act::Mcd => Flow::Cont,
            _ => return None,
        })
    }

    pub fn create_archive(&mut self, name: &str, items: Vec<String>, mv: bool) -> Flow {
        if name.is_empty() {
            return Flow::Cont;
        }
        let ext = split_ext(name.split_whitespace().last().unwrap_or(name)).1.to_ascii_uppercase();
        let kind = if ext.is_empty() { mdir_core::cfg::ARCHIVES[self.cfg.num("ARCHIVE").clamp(0, 10) as usize].0.to_string() } else { ext };
        if (kind == "ZIP" || kind == "JAR") && !name.starts_with('-') {
            let arc = self.dir.join(name);
            match zip_files(&arc, &self.dir, &items) {
                Ok(_) => {
                    if mv {
                        for it in &items {
                            let _ = crate::ops::remove(&self.dir.join(it));
                        }
                    }
                    self.reload();
                    if let Some(i) = self.entries.iter().position(|e| self.dir.join(&e.name) == arc) {
                        self.cur = i;
                    }
                }
                Err(_) => self.error(t("압축유틸리티를 실행할 수 없습니다", "Cannot run Archive program")),
            }
            return Flow::Cont;
        }
        let (_, add, mvc) = tools(&kind);
        let files: Vec<String> = items.iter().map(|s| if s.contains(' ') { format!("\"{}\"", s) } else { s.clone() }).collect();
        let cmd = format!("{} {} {}", if mv { mvc } else { add }, name, files.join(" "));
        self.launch(cmd, self.dir.clone(), true, false)
    }

    /// Extracts; returns the command to run (in `dest`) for kinds handled by their own program.
    pub fn extract(&mut self, arc: &Path, dest: &Path, inner: &[String], spec: &str) -> Option<String> {
        let kind = kind_of(arc).unwrap_or_default();
        if std::fs::create_dir_all(dest).is_err() {
            self.error(t("대상 디렉토리를 만들 수 없습니다", "Cannot creat dest-directory"));
            return None;
        }
        if kind == "ZIP" {
            match unzip(arc, dest, inner, spec) {
                Ok(_) => {
                    if dest != self.dir {
                        self.prev_dir = Some(dest.to_path_buf());
                    }
                    for e in self.entries.iter_mut() {
                        e.tag = 0;
                    }
                }
                Err(e) => self.error(&e),
            }
            None
        } else {
            let (x, _, _) = tools(&kind);
            Some(format!("{} \"{}\" {}", x, arc.display(), spec))
        }
    }
}
