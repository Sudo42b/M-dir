//! Reading a directory into the main-screen list: entries, filters, sorting and the per-mode
//! text of one entry.
use mdir_core::fsx::{self, Attr, Drive};
use mdir_core::pal::{self, Colors};
use mdir_core::text::{self, commas, fit, fit_right, name83, split_ext, t, wild};
use std::path::Path;
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Up,
    Dir,
    File,
    Drive,
    /// "<EXIT>" line at the top of an archive view.
    Exit,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    pub size: u64,
    pub mtime: SystemTime,
    pub attr: Attr,
    pub exec: bool,
    pub tag: u32,
    pub drive: Option<Drive>,
    /// Path inside an archive (archive view only).
    pub inner: String,
}

impl Entry {
    pub fn is_file(&self) -> bool {
        self.kind == Kind::File
    }
    pub fn is_dirlike(&self) -> bool {
        matches!(self.kind, Kind::Dir | Kind::Up)
    }
    pub fn ext(&self) -> &str {
        split_ext(&self.name).1
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    None,
    Name,
    Ext,
    Size,
    Time,
    Color,
    Exec,
}

impl Sort {
    pub fn from_index(i: i64) -> Sort {
        [Sort::None, Sort::Name, Sort::Ext, Sort::Size, Sort::Time, Sort::Color, Sort::Exec][i.clamp(0, 6) as usize]
    }
    pub fn index(self) -> i64 {
        self as i64
    }
    /// Letter shown on the count line (N E S T B Q), none for "no sort".
    pub fn letter(self) -> Option<char> {
        match self {
            Sort::None => None,
            Sort::Name => Some('N'),
            Sort::Ext => Some('E'),
            Sort::Size => Some('S'),
            Sort::Time => Some('T'),
            Sort::Color => Some('B'),
            Sort::Exec => Some('Q'),
        }
    }
}

pub struct ListOpts<'a> {
    pub hidden: bool,
    pub exe_only: bool,
    pub filter: &'a [String],
}

/// Reads `dir`. Drives come last, then the list is sorted (directories first, as MdirIII shows).
pub fn read_dir(dir: &Path, o: &ListOpts, drives: &[Drive]) -> std::io::Result<Vec<Entry>> {
    let mut v = vec![];
    if dir.parent().is_some() {
        let m = std::fs::metadata(dir.join("..")).ok();
        v.push(Entry {
            name: "..".into(),
            kind: Kind::Up,
            size: 0,
            mtime: m.and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH),
            attr: Attr::default(),
            exec: false,
            tag: 0,
            drive: None,
            inner: String::new(),
        });
    }
    for e in std::fs::read_dir(dir)?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(m) = e.metadata().or_else(|_| std::fs::symlink_metadata(e.path())) else { continue };
        let a = fsx::attr(&name, &m);
        if (a.hidden || a.system) && !o.hidden {
            continue;
        }
        let is_dir = m.is_dir();
        let exec = !is_dir && fsx::is_exec(&name, &m);
        if !is_dir {
            if o.exe_only && !exec {
                continue;
            }
            if !o.filter.is_empty() && !o.filter.iter().any(|f| wild(f, &name)) {
                continue;
            }
        }
        v.push(Entry {
            name,
            kind: if is_dir { Kind::Dir } else { Kind::File },
            size: if is_dir { 0 } else { m.len() },
            mtime: m.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            attr: a,
            exec,
            tag: 0,
            drive: None,
            inner: String::new(),
        });
    }
    for d in drives {
        v.push(Entry {
            name: fsx::drive_label(d),
            kind: Kind::Drive,
            size: 0,
            mtime: SystemTime::UNIX_EPOCH,
            attr: Attr::default(),
            exec: false,
            tag: 0,
            drive: Some(d.clone()),
            inner: String::new(),
        });
    }
    Ok(v)
}

fn group(e: &Entry) -> u8 {
    match e.kind {
        Kind::Exit => 0,
        Kind::Up => 1,
        Kind::Dir => 2,
        Kind::File => 3,
        Kind::Drive => 4,
    }
}

/// Sorts with MSET's key order: name (name, ext), ext (ext, name), size/time (key, name, ext),
/// colour/exec (key, ext, name). Descending reverses the first key (first two for colour/exec).
pub fn sort(v: &mut [Entry], s: Sort, desc: bool, colors: &Colors) {
    use std::cmp::Ordering;
    let key = |e: &Entry| (split_ext(&e.name).0.to_lowercase(), e.ext().to_lowercase());
    v.sort_by(|a, b| {
        let g = group(a).cmp(&group(b));
        if g != Ordering::Equal || s == Sort::None || a.kind == Kind::Drive {
            return g;
        }
        let (an, ae) = key(a);
        let (bn, be) = key(b);
        let rev = |o: Ordering| if desc { o.reverse() } else { o };
        match s {
            Sort::None => Ordering::Equal,
            Sort::Name => rev(an.cmp(&bn)).then(ae.cmp(&be)),
            Sort::Ext => rev(ae.cmp(&be)).then(an.cmp(&bn)),
            Sort::Size => rev(a.size.cmp(&b.size)).then(an.cmp(&bn)).then(ae.cmp(&be)),
            Sort::Time => rev(a.mtime.cmp(&b.mtime)).then(an.cmp(&bn)).then(ae.cmp(&be)),
            Sort::Color => rev(file_attr(a, colors).cmp(&file_attr(b, colors))).then(rev(ae.cmp(&be))).then(an.cmp(&bn)),
            Sort::Exec => rev(exec_rank(&ae).cmp(&exec_rank(&be))).then(ae.cmp(&be)).then(an.cmp(&bn)),
        }
    });
}

/// Exec-first order (M.EXE seg 114b0:319b, strings at 318b): BAT, BTM, COM, EXE, then the rest.
fn exec_rank(ext: &str) -> usize {
    ["bat", "btm", "com", "exe"].iter().position(|x| *x == ext).unwrap_or(4)
}

/// Colour of an entry's name: directories, drives, extension colours (hidden files only change the
/// colour of their size, see `size_attr`).
pub fn file_attr(e: &Entry, c: &Colors) -> u8 {
    let bg = c.items[pal::FILE] & 0xf0;
    match e.kind {
        Kind::Up | Kind::Dir | Kind::Exit => c.items[pal::DIR],
        Kind::Drive => c.items[pal::DRIVE],
        Kind::File => {
            if let Some(a) = c.ext_color(e.ext()) {
                return bg | (a & 0x0f);
            }
            if e.exec {
                return bg | 0x0a;
            }
            c.items[pal::FILE]
        }
    }
}

pub struct Fmt {
    pub lower: bool,
    pub attr: bool,
    pub h24: bool,
    pub drive_type: bool,
}

/// Text of one entry in a column `w` cells wide. Mode 1 adds attributes and the description.
pub fn entry_text(e: &Entry, w: usize, f: &Fmt, desc: Option<&str>, order: Option<u32>) -> String {
    if e.kind == Kind::Exit {
        let s = if e.name == "<EXIT>" { "<EXIT>" } else { "<Exit>" };
        return fit(s, w);
    }
    if e.kind == Kind::Drive {
        let d = e.drive.as_ref().unwrap();
        let mut s = format!("[-{}-]", fsx::drive_label(d));
        if f.drive_type && w >= 12 {
            s += &format!(" {}", d.kind);
        }
        if w >= 19 && f.drive_type {
            s += &format!("  {}", d.root.to_string_lossy());
        }
        return fit(&s, w);
    }
    let name = if e.kind == Kind::Up { fit("..", 12) } else { name83(&e.name, f.lower && e.kind == Kind::File) };
    let size = |n: usize| size_field(e, n);
    let date = text::fmt_date(e.mtime);
    let time = text::fmt_time(e.mtime, f.h24);
    if w >= 70 {
        // mode 1
        let mut s = format!("{}{} {} {}", name, size(11), date, time);
        if f.attr || e.attr.readonly || e.attr.hidden {
            s += &format!(" {}", e.attr.text());
        }
        if let Some(o) = order {
            s += &format!(" #{:<3}", o);
        }
        if let Some(d) = desc {
            s += &format!("  {}", d);
        }
        return fit(&s, w);
    }
    if w >= 38 {
        let last = if f.attr { e.attr.text() } else { time };
        return fit(&format!("{}{} {} {}", name, size(11), date, last), w);
    }
    if w >= 24 {
        return fit(&format!("{} {}", name, size(11)), w);
    }
    if w >= 19 {
        return fit(&format!("{}{}", name, size(7)), w);
    }
    fit(&name, w)
}

/// Size column, as M.EXE draws it (seg 114b0:0633): 11 cells with commas, or "{MB}M" from
/// 1,000,000,000 bytes; in the 7-cell mode 4 plain digits below 1,000,000, else tenths of a MB
/// ("  12.3M"). Directories read " [ SubDir ]" / " [ Up-Dir ]", or "   DIR " in mode 4.
pub fn size_field(e: &Entry, n: usize) -> String {
    if e.is_dirlike() {
        return if n < 11 { "   DIR ".into() } else if e.kind == Kind::Up { " [ Up-Dir ]".into() } else { " [ SubDir ]".into() };
    }
    if n < 11 {
        if e.size < 1_000_000 {
            return format!("{:>7}", e.size);
        }
        let mut v = format!("{:>5}", e.size / 104_857);
        v.insert(4, '.');
        if v.as_bytes()[3] == b' ' {
            v.replace_range(3..4, "0");
        }
        return format!("{}M", v);
    }
    if e.size > 999_999_999 {
        format!("{:>10}M", e.size >> 20)
    } else {
        fit_right(&commas(e.size), 11)
    }
}

/// Colour of the size column (seg 114b0:0633): the "other files" colour; yellow for read-only or
/// system files, bright magenta for hidden ones.
pub fn size_attr(e: &Entry, c: &Colors) -> u8 {
    let a = c.items[pal::FILE];
    if e.kind != Kind::File || !(e.attr.readonly || e.attr.hidden || e.attr.system) {
        return a;
    }
    (a & 0xf0) | if e.attr.hidden { 0x0d } else { 0x0e }
}

/// Count line (seg 114b0:29bf): files and dirs as Str(n:5), bytes and free bytes right-aligned in 14
/// cells with commas, the free percentage as free div (total div 100), the disk size in K up to
/// 0x200B20 bytes and in M above. Numbers (between ^) use the highlight colour.
pub fn count_line(files: usize, dirs: usize, bytes: u64, space: Option<(u64, u64)>) -> String {
    let mut s = format!("^{:>5}^ File^{:>5}^ Dir^{}^ Byte ", files, dirs, fit_right(&commas(bytes), 14));
    if let Some((total, free)) = space {
        let pct = if total >= 100 { free / (total / 100) } else { 0 };
        let tot = if total > 0x20_0B20 { format!("{}^M", total >> 20) } else { format!("{}^K", total >> 10) };
        s += &format!("^{}^(^{}^%{}^{})", fit_right(&commas(free), 14), pct, t(")byte 남음 (", ")byte free ("), tot);
    }
    s
}
