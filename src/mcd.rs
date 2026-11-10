//! MCD (MdirIII Change Directory): the whole directory tree of a drive drawn sideways, with
//! directory move/copy/rename/make/delete/size and a cached tree file per drive.
use crate::app::{App, Mode, Pending};
use mdir_core::fsx;
use mdir_core::modal::{alt, ctrl, shift, Modal};
use mdir_core::pal::{self, Colors};
use mdir_core::ratatui::buffer::Buffer;
use mdir_core::ratatui::crossterm::event::{KeyCode, KeyEvent};
use mdir_core::scr::{fill, put, put_hl, putw};
use mdir_core::term::Flow;
use mdir_core::text::{commas, fit, t};
use std::path::{Path, PathBuf};

pub const MAX_DIRS: usize = 20000;
const UNIT: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Purpose {
    Go,
    /// Put the chosen directory into the input box that called F10.
    Insert,
    QcdSet(usize),
}

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub depth: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub row: usize,
}

pub struct Mcd {
    pub root: PathBuf,
    pub nodes: Vec<Node>,
    pub cur: usize,
    pub top: usize,
    pub search: Option<String>,
    pub purpose: Purpose,
    pub qcd_armed: Option<usize>,
    pub hidden: bool,
}

pub enum McdRes {
    Keep,
    Close,
    Go(PathBuf),
}

fn cache_file(root: &Path) -> PathBuf {
    let tag: String = root.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    mdir_core::term::data_dir().join(format!("MDIRTREE{}.DIR", tag))
}

fn skip_dir(p: &Path) -> bool {
    !cfg!(windows) && ["/proc", "/sys", "/dev", "/run", "/snap", "/tmp/.X11-unix"].iter().any(|s| p == Path::new(s))
}

impl Mcd {
    pub fn path_of(&self, i: usize) -> PathBuf {
        let mut parts = vec![];
        let mut n = Some(i);
        while let Some(k) = n {
            if k != 0 {
                parts.push(self.nodes[k].name.clone());
            }
            n = self.nodes[k].parent;
        }
        let mut p = self.root.clone();
        for s in parts.iter().rev() {
            p.push(s);
        }
        p
    }

    /// Breadth-first, so the upper levels are complete when the size limit is reached.
    pub fn scan(root: &Path, hidden: bool, sort: bool) -> Vec<Node> {
        let mut nodes = vec![Node { name: root.to_string_lossy().to_string(), depth: 0, parent: None, children: vec![], row: 0 }];
        let mut queue = std::collections::VecDeque::from([(0usize, root.to_path_buf())]);
        while let Some((i, p)) = queue.pop_front() {
            if nodes.len() >= MAX_DIRS {
                break;
            }
            let Ok(rd) = std::fs::read_dir(&p) else { continue };
            let mut subs: Vec<String> = rd
                .flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir() && !t.is_symlink()).unwrap_or(false))
                .map(|e| e.file_name().to_string_lossy().to_string())
                .filter(|n| hidden || !n.starts_with('.'))
                .collect();
            if sort {
                subs.sort_by_key(|s| s.to_lowercase());
            }
            for s in subs {
                let cp = p.join(&s);
                if skip_dir(&cp) || nodes.len() >= MAX_DIRS {
                    continue;
                }
                let k = nodes.len();
                let d = nodes[i].depth + 1;
                nodes.push(Node { name: s, depth: d, parent: Some(i), children: vec![], row: 0 });
                nodes[i].children.push(k);
                queue.push_back((k, cp));
            }
        }
        nodes
    }

    /// Adds the directories from the root down to `p` when the size limit left them out.
    pub fn ensure_path(&mut self, p: &Path) {
        let Ok(rel) = p.strip_prefix(&self.root) else { return };
        let mut i = 0;
        for c in rel.components() {
            let s = c.as_os_str().to_string_lossy().to_string();
            i = match self.nodes[i].children.iter().copied().find(|&k| self.nodes[k].name == s) {
                Some(k) => k,
                None => {
                    let k = self.nodes.len();
                    let d = self.nodes[i].depth + 1;
                    self.nodes.push(Node { name: s, depth: d, parent: Some(i), children: vec![], row: 0 });
                    self.nodes[i].children.push(k);
                    k
                }
            };
        }
        self.layout();
    }

    fn save_cache(&self) {
        fn out(n: &[Node], i: usize, s: &mut String) {
            for &c in &n[i].children {
                *s += &format!("{}\t{}\n", n[c].depth, n[c].name);
                out(n, c, s);
            }
        }
        let mut s = String::new();
        out(&self.nodes, 0, &mut s);
        let p = cache_file(&self.root);
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, s);
    }

    fn load_cache(root: &Path) -> Option<Vec<Node>> {
        let s = std::fs::read_to_string(cache_file(root)).ok()?;
        let mut nodes = vec![Node { name: root.to_string_lossy().to_string(), depth: 0, parent: None, children: vec![], row: 0 }];
        let mut stack = vec![0usize];
        for line in s.lines() {
            let (d, name) = line.split_once('\t')?;
            let d: usize = d.parse().ok()?;
            if d == 0 || d > stack.len() {
                return None;
            }
            stack.truncate(d);
            let parent = *stack.last()?;
            let k = nodes.len();
            nodes.push(Node { name: name.to_string(), depth: d, parent: Some(parent), children: vec![], row: 0 });
            nodes[parent].children.push(k);
            stack.push(k);
        }
        Some(nodes)
    }

    fn layout(&mut self) {
        fn go(n: &mut Vec<Node>, i: usize, row: usize) -> usize {
            n[i].row = row;
            let kids = n[i].children.clone();
            if kids.is_empty() {
                return row + 1;
            }
            let mut r = row;
            for c in kids {
                r = go(n, c, r);
            }
            r
        }
        go(&mut self.nodes, 0, 0);
    }

    pub fn open(app: &App, p: Purpose) -> Mcd {
        let root = fsx::drive_of(&app.dir, &app.drives).map(|d| d.root).unwrap_or_else(|| PathBuf::from("/"));
        let mut m = Mcd { root: root.clone(), nodes: vec![], cur: 0, top: 0, search: None, purpose: p, qcd_armed: None, hidden: app.hidden };
        m.nodes = Mcd::load_cache(&root).unwrap_or_default();
        m.layout_or_scan(app.cfg.flag("MCDSORT"), false);
        // automatic rescan when the current directory is not in the saved tree
        if !m.goto_path(&app.dir) {
            m.rescan(app.cfg.flag("MCDSORT"));
            if !m.goto_path(&app.dir) {
                m.ensure_path(&app.dir);
                m.goto_path(&app.dir);
            }
        }
        m
    }

    fn layout_or_scan(&mut self, sort: bool, force: bool) {
        if self.nodes.is_empty() || force {
            self.nodes = Mcd::scan(&self.root, self.hidden, sort);
            self.save_cache();
        }
        self.layout();
    }

    pub fn rescan(&mut self, sort: bool) {
        let here = self.path_of(self.cur.min(self.nodes.len().saturating_sub(1)));
        self.layout_or_scan(sort, true);
        self.cur = 0;
        self.goto_path(&here);
    }

    pub fn sort_by_name(&mut self) {
        let names: Vec<String> = self.nodes.iter().map(|n| n.name.to_lowercase()).collect();
        for n in self.nodes.iter_mut() {
            n.children.sort_by(|a, b| names[*a].cmp(&names[*b]));
        }
        self.layout();
    }

    /// Moves the bar to `p`; false when it is not in the tree.
    pub fn goto_path(&mut self, p: &Path) -> bool {
        let Ok(rel) = p.strip_prefix(&self.root) else { return false };
        let mut i = 0;
        for c in rel.components() {
            let s = c.as_os_str().to_string_lossy();
            match self.nodes[i].children.iter().find(|&&k| self.nodes[k].name == s) {
                Some(&k) => i = k,
                None => return false,
            }
        }
        self.cur = i;
        true
    }

    /// Nodes in screen (row) order.
    fn order(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.nodes.len()).collect();
        v.sort_by_key(|&i| (self.nodes[i].row, self.nodes[i].depth));
        v
    }

    pub fn find(&mut self, s: &str, from_next: bool) -> bool {
        let ord = self.order();
        let pos = ord.iter().position(|&i| i == self.cur).unwrap_or(0);
        let s = s.to_lowercase();
        for k in 0..ord.len() {
            let i = ord[(pos + k + usize::from(from_next)) % ord.len()];
            if self.nodes[i].name.to_lowercase().starts_with(&s) && i != 0 {
                self.cur = i;
                return true;
            }
        }
        false
    }

    pub fn key(&mut self, k: KeyEvent, app: &mut App) -> McdRes {
        let n = &self.nodes;
        let cur = self.cur;
        let sib = |d: isize| -> Option<usize> {
            let p = n[cur].parent?;
            let ks = &n[p].children;
            let i = ks.iter().position(|&x| x == cur)? as isize + d;
            ks.get(usize::try_from(i).ok()?).copied()
        };
        if let Some(s) = self.search.clone() {
            match k.code {
                KeyCode::Esc => {
                    self.search = None;
                    return McdRes::Keep;
                }
                KeyCode::Backspace => {
                    let mut s = s;
                    s.pop();
                    self.search = if s.is_empty() { None } else { Some(s) };
                    return McdRes::Keep;
                }
                KeyCode::Char(' ') => {
                    self.find(&s, true);
                    return McdRes::Keep;
                }
                KeyCode::Char('l') if ctrl(&k) => {
                    self.find(&s, true);
                    return McdRes::Keep;
                }
                _ => {}
            }
        }
        let sort = app.cfg.flag("MCDSORT");
        match k.code {
            KeyCode::Esc => return McdRes::Close,
            KeyCode::Enter => return McdRes::Go(self.path_of(self.cur)),
            KeyCode::Up => self.cur = sib(-1).unwrap_or(cur),
            KeyCode::Down => self.cur = sib(1).unwrap_or(cur),
            KeyCode::Left if ctrl(&k) => self.step(-1),
            KeyCode::Right if ctrl(&k) => self.step(1),
            KeyCode::Left => self.cur = n[cur].parent.unwrap_or(cur),
            KeyCode::Right => self.cur = n[cur].children.first().copied().unwrap_or(cur),
            KeyCode::Home | KeyCode::Char('/') => self.cur = 0,
            KeyCode::End => self.cur = *self.order().last().unwrap_or(&0),
            KeyCode::PageUp | KeyCode::PageDown => {
                let rows = app.area_h.saturating_sub(4) as usize;
                let ord = self.order();
                let row = n[cur].row;
                let first = self.top;
                let lastr = self.top + rows - 1;
                let target = if k.code == KeyCode::PageUp {
                    if row > first { first } else { row.saturating_sub(rows) }
                } else if row < lastr { lastr } else { row + rows };
                let pick = if k.code == KeyCode::PageUp {
                    ord.iter().copied().find(|&i| n[i].row >= target)
                } else {
                    ord.iter().rev().copied().find(|&i| n[i].row <= target)
                };
                self.cur = pick.unwrap_or(cur);
            }
            KeyCode::Char('n') if alt(&k) => self.sort_by_name(),
            KeyCode::F(1) if !shift(&k) => {
                app.act(crate::act::Act::Help);
                if let Mode::Help { .. } = app.mode {
                    // help replaces MCD; Esc from help returns to the main screen
                }
                return McdRes::Keep;
            }
            KeyCode::F(2) => self.rescan(sort),
            KeyCode::F(3) => {
                let items = app.drives.iter().map(|d| format!(" [-{}-] {}  {}", fsx::drive_label(d), d.kind, d.root.display())).collect();
                app.modal = Some((Modal::menu(t("드라이브선택", "Drive"), items, 0), Pending::McdDrive));
            }
            KeyCode::F(n @ 4..=5) => {
                let op = if n == 4 { crate::ops::Op::Move } else { crate::ops::Op::Copy };
                let what = if n == 4 { t("이동", "MOVE") } else { t("복사", "COPY") };
                let m = Modal::input(&format!("<{}>", what), &[&format!("{} -> ", self.path_of(cur).display())], &self.path_of(self.nodes[cur].parent.unwrap_or(0)).to_string_lossy());
                if cur != 0 {
                    app.modal = Some((m, Pending::McdCopy { op }));
                }
            }
            KeyCode::F(6) => {
                if cur != 0 {
                    let m = Modal::input(t("디렉토리의 새이름은?", "Rename Directory"), &[&self.nodes[cur].name.clone()], &self.nodes[cur].name.clone());
                    app.modal = Some((m, Pending::McdRename));
                }
            }
            KeyCode::F(7) | KeyCode::Insert => {
                let m = Modal::input(t("디렉토리  만들기", "Make Directory"), &[&self.path_of(cur).display().to_string()], "");
                app.modal = Some((m, Pending::McdMkdir));
            }
            KeyCode::F(8) | KeyCode::Delete => {
                if k.code == KeyCode::Delete && !app.cfg.flag("DELKEY") {
                    return McdRes::Keep;
                }
                let p = self.path_of(cur);
                let empty = std::fs::read_dir(&p).map(|mut r| r.next().is_none()).unwrap_or(false);
                if cur == 0 {
                    if !app.cfg.flag("ROOTDEL") {
                        return McdRes::Keep;
                    }
                    let m = Modal::yes_no(t("지우기", "Delete"), &[&format!("{} {}", p.display(), t(":의 모든 자료를 삭제합니까?", ": will be lost !!!"))], false);
                    app.modal = Some((m, Pending::McdDelete { left: 3 }));
                } else if !empty && !app.cfg.flag("DIRDEL") {
                    app.error(t("빈 디렉토리가 아닙니다", "Not Empty"));
                } else {
                    let msg = if empty { t("지울까요?", "Delete?") } else { t("빈 디렉토리가 아닙니다. 지울까요?", "Not empty!  Delete?") };
                    let m = Modal::yes_no(t("지우기", "Delete"), &[&p.display().to_string(), msg], app.cfg.flag("DEL_YES"));
                    app.modal = Some((m, Pending::McdDelete { left: 1 }));
                }
            }
            KeyCode::F(9) => {
                let p = self.path_of(cur);
                let (mut files, mut dirs, mut bytes, mut alloc) = (0u64, 0u64, 0u64, 0u64);
                fn walk(p: &Path, f: &mut u64, d: &mut u64, b: &mut u64, a: &mut u64) {
                    if let Ok(rd) = std::fs::read_dir(p) {
                        for e in rd.flatten() {
                            let Ok(ft) = e.file_type() else { continue };
                            if ft.is_dir() {
                                *d += 1;
                                walk(&e.path(), f, d, b, a);
                            } else if let Ok(m) = e.metadata() {
                                *f += 1;
                                *b += m.len();
                                *a += m.len().div_ceil(4096) * 4096;
                            }
                        }
                    }
                }
                walk(&p, &mut files, &mut dirs, &mut bytes, &mut alloc);
                let (total, free) = fsx::disk_space(&p).unwrap_or((0, 0));
                let lines = [
                    format!("{}{:>15}{}", t("자식 디렉토리수  : ", "Number of SubDir : "), commas(dirs), " Dirs"),
                    format!("{}{:>15}{}", t("파일의 수        : ", "Number of files  : "), commas(files), " Files"),
                    format!("{}{:>15}{}", t("파일들의 총크기  : ", "Size of files    : "), commas(bytes), " Bytes"),
                    format!("{}{:>15}{}", t("할당된 총 크기   : ", "Allocation space : "), commas(alloc), " Bytes"),
                    format!("{}{:>15}{}", t("디스크 총용량    : ", "Total space      : "), commas(total), " Bytes"),
                    format!("{}{:>15}{}", t("디스크 남은용량  : ", "Free  space      : "), commas(free), " Bytes"),
                    format!("{}{:>15}{}", t("파일 할당단위    : ", "Allocation unit  : "), commas(4096), " Bytes"),
                ];
                let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
                app.info("Check Tree Size", &refs);
            }
            KeyCode::F(11) => app.open_qcd(false),
            KeyCode::F(n) if shift(&k) && n <= 10 => {
                let i = n as usize - 1;
                let d = app.cfg.qcd[i].clone();
                if !d.is_empty() {
                    if self.qcd_armed == Some(i) {
                        return McdRes::Go(PathBuf::from(d));
                    }
                    self.goto_path(Path::new(&d));
                    self.qcd_armed = Some(i);
                    return McdRes::Keep;
                }
            }
            KeyCode::Char(' ') if self.search.is_none() => {
                // visit the QCD directories in turn
                let list: Vec<String> = app.cfg.qcd.iter().filter(|s| !s.is_empty()).cloned().collect();
                if !list.is_empty() {
                    let here = self.path_of(cur);
                    let pos = list.iter().position(|d| Path::new(d) == here).map(|p| p + 1).unwrap_or(0);
                    let d = list[pos % list.len()].clone();
                    if !self.goto_path(Path::new(&d)) {
                        app.cfg.qcd.iter_mut().filter(|q| **q == d).for_each(|q| q.clear());
                        let _ = app.cfg.save();
                    }
                }
            }
            KeyCode::Char(c) if !ctrl(&k) && !alt(&k) => {
                if c.is_ascii_uppercase() {
                    if let Some(d) = app.drives.iter().find(|d| d.letter == c).cloned() {
                        self.change_root(d.root, sort);
                    }
                    return McdRes::Keep;
                }
                let mut s = self.search.clone().unwrap_or_default();
                s.push(c);
                self.find(&s, false);
                self.search = Some(s);
            }
            _ => {}
        }
        if !matches!(k.code, KeyCode::F(_)) {
            self.qcd_armed = None;
        }
        McdRes::Keep
    }

    pub fn change_root(&mut self, root: PathBuf, sort: bool) {
        self.root = root.clone();
        self.nodes = Mcd::load_cache(&root).unwrap_or_default();
        self.layout_or_scan(sort, false);
        self.cur = 0;
    }

    fn step(&mut self, d: isize) {
        let ord = self.order();
        let p = ord.iter().position(|&i| i == self.cur).unwrap_or(0) as isize + d;
        if p >= 0 && (p as usize) < ord.len() {
            self.cur = ord[p as usize];
        }
    }

    pub fn draw(&mut self, b: &mut Buffer, c: &Colors, dir_label: &str) {
        let area = b.area;
        let (w, h) = (area.width as usize, area.height as usize);
        let st = c.items[pal::MCD_STATUS];
        let hi = (st & 0xf0) | 0x01;
        let tree = c.items[pal::MCD];
        fill(b, 0, 0, area.width, area.height, ' ', tree);
        putw(b, 0, 0, "", area.width, st);
        let title = match self.purpose {
            Purpose::Go => t(" 디렉토리 관리기(^MCD^) ", " ^M^dirIII ^C^hange ^D^irectory ").to_string(),
            _ => format!("{}{}^]  ^SPACE^={}", t(" 디렉토리를 선택하세요 [^", " select path [^"), fit(&self.path_of(self.cur).to_string_lossy(), 30).trim_end(), t("다음찾기", "next match")),
        };
        put_hl(b, ((w as u16).saturating_sub(mdir_core::scr::hw(&title))) / 2, 0, &title, st, hi);
        // path / speed-search line
        let sline = c.items[pal::SEARCH_MCD];
        putw(b, 0, 1, "", area.width, tree);
        let path = self.path_of(self.cur).display().to_string();
        put(b, 1, 1, &fit(&path, w.saturating_sub(24)), tree);
        if let Some(s) = &self.search {
            putw(b, (w as u16).saturating_sub(22), 1, &format!("[{}_]", s), 21, sline);
        }
        let rows = h.saturating_sub(4);
        let crow = self.nodes[self.cur].row;
        // keep the bar in view, centred when it has to jump
        if crow < self.top || crow >= self.top + rows {
            self.top = crow.saturating_sub(rows / 2);
        }
        let cx = self.nodes[self.cur].depth * UNIT;
        let left = if cx + UNIT + 2 > w { (cx + UNIT + 2 - w).div_ceil(UNIT) * UNIT } else { 0 };
        let y0 = 2;
        let mut set = |x: usize, row: usize, s: &str, a: u8| {
            if row >= self.top && row < self.top + rows && x >= left && x - left < w {
                put(b, (x - left) as u16, (y0 + row - self.top) as u16, s, a);
            }
        };
        for (i, nd) in self.nodes.iter().enumerate() {
            let x = nd.depth * UNIT;
            let shown = fit(&nd.name, 12);
            let a = if i == self.cur { pal::bar(tree) } else { tree };
            set(x, nd.row, &shown, a);
            if !nd.children.is_empty() {
                let last = *nd.children.last().unwrap();
                set(x + 12, nd.row, if nd.children.len() > 1 { "─┬──" } else { "────" }, tree);
                let lr = self.nodes[last].row;
                for r in (nd.row + 1)..=lr {
                    let is_child = nd.children.iter().any(|&k| self.nodes[k].row == r);
                    let ch = if r == lr { " └──" } else if is_child { " ├──" } else { " │" };
                    set(x + 12, r, ch, tree);
                }
            }
        }
        // function key line
        let fy = (h - 1) as u16;
        putw(b, 0, fy, "", area.width, st);
        let keys = if mdir_core::text::korean() {
            "^F1^도움 ^F2^재검색 ^F3^드라이브 ^F4^이동 ^F5^복사 ^F6^이름 ^F7^만듦 ^F8^삭제 ^F9^크기"
        } else {
            "^F1^-Help ^F2^-Rescan ^F3^-Drive ^F4^-Move ^F5^-Copy ^F6^-Rename ^F7^-Mkdir ^F8^-Delete ^F9^-Size"
        };
        let x = put_hl(b, 0, fy, &format!("^[{}]^ ", fit(dir_label, 8)), st, hi);
        put_hl(b, x, fy, keys, st, hi);
    }
}

impl App {
    pub fn mcd_result(&mut self, mcd: Box<Mcd>, r: McdRes) -> Flow {
        match r {
            McdRes::Keep => {
                if matches!(self.mode, Mode::Main) {
                    self.mode = Mode::Mcd(mcd);
                }
            }
            McdRes::Close => {
                if self.quit_mcd {
                    return Flow::Quit;
                }
                if mcd.purpose == Purpose::Insert {
                    self.modal = self.stash.pop();
                } else if let Purpose::QcdSet(_) = mcd.purpose {
                    self.open_qcd(false);
                }
            }
            McdRes::Go(p) => match mcd.purpose {
                Purpose::Go => {
                    self.chdir(p);
                    if self.quit_mcd {
                        return Flow::Quit;
                    }
                }
                Purpose::Insert => {
                    if let Some((mdir_core::modal::Modal::Input { edit, .. }, _)) = self.stash.last_mut() {
                        if edit.fresh {
                            edit.set(&p.to_string_lossy());
                            edit.fresh = false;
                        } else {
                            edit.insert_str(&p.to_string_lossy());
                        }
                    }
                    self.modal = self.stash.pop();
                }
                Purpose::QcdSet(i) => {
                    self.cfg.qcd[i] = p.to_string_lossy().to_string();
                    let _ = self.cfg.save();
                    self.open_qcd(false);
                }
            },
        }
        Flow::Cont
    }

    /// Answers to the boxes MCD opened.
    pub fn mcd_text(&mut self, p: &Pending, s: &str) {
        let Mode::Mcd(m) = &mut self.mode else { return };
        let cur = m.cur;
        let path = m.path_of(cur);
        let sort = self.cfg.flag("MCDSORT");
        let mut err = None;
        match p {
            Pending::McdMkdir => {
                if !s.trim().is_empty() {
                    if std::fs::create_dir_all(path.join(s.trim())).is_err() {
                        err = Some(t("디렉토리를 만들 수 없습니다", "Unable to creat directory"));
                    }
                    m.rescan(sort);
                    m.goto_path(&path.join(s.trim()));
                }
            }
            Pending::McdRename => {
                let to = path.with_file_name(s.trim());
                if s.trim().is_empty() || to.exists() || std::fs::rename(&path, &to).is_err() {
                    err = Some(t("이름이 잘못되었거나 이미 존재합니다", "Already exist or Bad file name"));
                } else {
                    m.rescan(sort);
                    m.goto_path(&to);
                }
            }
            Pending::McdCopy { op } => {
                let dest = Path::new(s.trim()).join(path.file_name().unwrap_or_default());
                if dest.starts_with(&path) {
                    err = Some(t("자신의 디렉토리로 트리를 복사할 수 없습니다", "Cannot COPY/MOVE tree into itself"));
                } else if dest.exists() {
                    err = Some(t("이미 존재합니다", "Already exist"));
                } else {
                    let r = if *op == crate::ops::Op::Move { crate::ops::move_tree(&path, &dest) } else { crate::ops::copy_tree(&path, &dest) };
                    if r.is_err() {
                        err = Some(t("복사/이동 실패", "COPY/MOVE failure"));
                    }
                    m.rescan(sort);
                    m.goto_path(&dest);
                }
            }
            _ => {}
        }
        if let Some(e) = err {
            self.error(e);
        }
    }

    pub fn mcd_choice(&mut self, p: Pending, c: usize) {
        let Mode::Mcd(m) = &mut self.mode else { return };
        match p {
            Pending::McdDrive => {
                if let Some(d) = self.drives.get(c) {
                    m.change_root(d.root.clone(), self.cfg.flag("MCDSORT"));
                }
            }
            Pending::McdDelete { left } => {
                if c != 0 {
                    return;
                }
                if left > 1 {
                    let mm = Modal::yes_no(t("지우기", "Delete"), &[t("정말로 지울까요?", "Really delete?"), &format!("({})", left - 1)], false);
                    self.modal = Some((mm, Pending::McdDelete { left: left - 1 }));
                    return;
                }
                let path = m.path_of(m.cur);
                let parent = m.nodes[m.cur].parent.unwrap_or(0);
                let ok = if m.cur == 0 {
                    std::fs::read_dir(&path).map(|r| r.flatten().all(|e| crate::ops::remove(&e.path()).is_ok())).unwrap_or(false)
                } else {
                    crate::ops::remove(&path).is_ok()
                };
                let back = m.path_of(parent);
                m.rescan(self.cfg.flag("MCDSORT"));
                m.goto_path(&back);
                if !ok {
                    self.error(t("지우기 실패", "Delete failure"));
                }
            }
            _ => {}
        }
    }
}
