//! File attributes, drives and DESCRIPT.ION (4DOS-compatible descriptions).
use std::collections::HashMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Attr {
    pub archive: bool,
    pub readonly: bool,
    pub hidden: bool,
    pub system: bool,
}

impl Attr {
    /// "A.R." style, as on the status line ('.' when off).
    pub fn text(&self) -> String {
        format!(
            "{}{}{}{}",
            if self.archive { 'A' } else { '.' },
            if self.readonly { 'R' } else { '.' },
            if self.hidden { 'H' } else { '.' },
            if self.system { 'S' } else { '.' }
        )
    }
}

pub fn attr(name: &str, m: &Metadata) -> Attr {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let a = m.file_attributes();
        let _ = name;
        Attr { archive: a & 0x20 != 0, readonly: a & 1 != 0, hidden: a & 2 != 0, system: a & 4 != 0 }
    }
    #[cfg(not(windows))]
    {
        Attr { archive: !m.is_dir(), readonly: m.permissions().readonly(), hidden: name.starts_with('.') || name == "DESCRIPT.ION", system: false }
    }
}

pub const EXEC_EXT: [&str; 5] = ["EXE", "COM", "BAT", "BTM", "CMD"];

pub fn is_exec(name: &str, m: &Metadata) -> bool {
    let (_, e) = crate::text::split_ext(name);
    if EXEC_EXT.iter().any(|x| x.eq_ignore_ascii_case(e)) {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if m.is_file() && m.permissions().mode() & 0o111 != 0 {
            return true;
        }
    }
    let _ = m;
    false
}

#[derive(Clone, Debug)]
pub struct Drive {
    /// Letter shown as [-C-]; '/' for the Unix root.
    pub letter: char,
    pub root: PathBuf,
    pub kind: &'static str,
}

/// Drives: letters on Windows; on other systems the root plus single-letter mounts (/mnt/c ...).
pub fn drives() -> Vec<Drive> {
    let mut v = vec![];
    if cfg!(windows) {
        for l in b'A'..=b'Z' {
            let root = PathBuf::from(format!("{}:\\", l as char));
            if root.exists() {
                v.push(Drive { letter: l as char, root, kind: if l < b'C' { "3.5\"" } else { "HDD" } });
            }
        }
    } else {
        v.push(Drive { letter: '/', root: PathBuf::from("/"), kind: "HDD" });
        for base in ["/mnt", "/media"] {
            if let Ok(rd) = std::fs::read_dir(base) {
                let mut ls: Vec<_> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.len() == 1 && n.chars().all(|c| c.is_ascii_alphabetic())).collect();
                ls.sort();
                for n in ls {
                    let c = n.chars().next().unwrap().to_ascii_uppercase();
                    if !v.iter().any(|d| d.letter == c) {
                        v.push(Drive { letter: c, root: Path::new(base).join(&n), kind: "HDD" });
                    }
                }
            }
        }
    }
    v
}

/// The drive a path is on (longest matching root).
pub fn drive_of(p: &Path, ds: &[Drive]) -> Option<Drive> {
    ds.iter().filter(|d| p.starts_with(&d.root)).max_by_key(|d| d.root.as_os_str().len()).cloned()
}

pub fn drive_label(d: &Drive) -> String {
    if d.letter == '/' {
        "/".into()
    } else {
        d.letter.to_string()
    }
}

/// (total, free) bytes of the disk holding `p`.
pub fn disk_space(p: &Path) -> Option<(u64, u64)> {
    Some((fs2::total_space(p).ok()?, fs2::available_space(p).ok()?))
}

pub const DESC_FILE: &str = "DESCRIPT.ION";

/// Descriptions of one directory, keyed by lower-case file name.
#[derive(Default, Clone, Debug)]
pub struct Descs {
    pub map: HashMap<String, String>,
}

impl Descs {
    pub fn load(dir: &Path) -> Descs {
        let mut d = Descs::default();
        let Ok(bytes) = std::fs::read(dir.join(DESC_FILE)) else { return d };
        let text = crate::text::decode_as(&bytes, crate::text::detect(&bytes));
        for line in text.lines() {
            let line = line.trim_end();
            let (name, rest) = if let Some(r) = line.strip_prefix('"') {
                match r.split_once('"') {
                    Some((n, rest)) => (n.to_string(), rest.trim_start().to_string()),
                    None => continue,
                }
            } else {
                match line.split_once(' ') {
                    Some((n, r)) => (n.to_string(), r.trim_start().to_string()),
                    None => continue,
                }
            };
            if !name.is_empty() && !rest.is_empty() {
                d.map.insert(name.to_lowercase(), rest);
            }
        }
        d
    }
    pub fn get(&self, name: &str) -> Option<&str> {
        self.map.get(&name.to_lowercase()).map(|s| s.as_str())
    }
    pub fn set(&mut self, name: &str, text: &str) {
        if text.trim().is_empty() {
            self.map.remove(&name.to_lowercase());
        } else {
            self.map.insert(name.to_lowercase(), text.trim().to_string());
        }
    }
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let p = dir.join(DESC_FILE);
        if self.map.is_empty() {
            let _ = std::fs::remove_file(&p);
            return Ok(());
        }
        let mut names: Vec<_> = self.map.keys().collect();
        names.sort();
        let mut s = String::new();
        for n in names {
            let shown = if n.contains(' ') { format!("\"{}\"", n) } else { n.clone() };
            s += &format!("{} {}\r\n", shown, self.map[n]);
        }
        std::fs::write(&p, s)
    }
}

/// Moves (or copies) the description of `name` in `from` to `to_name` in `to`.
pub fn carry_desc(from: &Path, name: &str, to: &Path, to_name: &str, remove_src: bool) {
    let mut src = Descs::load(from);
    let Some(text) = src.get(name).map(|s| s.to_string()) else { return };
    if remove_src {
        src.set(name, "");
        let _ = src.save(from);
    }
    let mut dst = Descs::load(to);
    dst.set(to_name, &text);
    let _ = dst.save(to);
}
