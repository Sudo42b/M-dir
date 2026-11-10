//! Main-screen state and the commands behind every key.
use crate::act::{key_act, menus, Act, MenuDef};
use crate::arc::ArcView;
use crate::list::{self, Entry, Fmt, Kind, ListOpts, Sort};
use crate::mcd::{Mcd, Purpose};
use crate::ops::{Job, Op};
use mdir_core::cfg::{alternatives, expand, Config, VarCtx};
use mdir_core::fsx::{self, Descs, Drive};
use mdir_core::modal::{alt, ctrl, shift, MRes, Modal};
use mdir_core::ratatui::crossterm::event::{KeyCode, KeyEvent};
use mdir_core::term::{Flow, RunReq};
use mdir_core::text::{t, wild};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub enum Pending {
    Info,
    DosCmd,
    Param { quit: bool },
    ProgChoice { alts: Vec<String>, quit: bool, edit: bool },
    EditCmd { quit: bool, pause: bool },
    Filter,
    Mkdir,
    Rename { names: Vec<String>, i: usize },
    Describe { names: Vec<String> },
    SelWild { select: bool },
    Dest { op: Op, items: Vec<String> },
    ConfirmDelete { items: Vec<String> },
    Job,
    JobRename,
    Qcd { insert: bool },
    History,
    ArcName { mv: bool, items: Vec<String> },
    ExtractDest { archives: Vec<PathBuf>, inner: Vec<String> },
    ExtractSpec { archives: Vec<PathBuf>, dest: PathBuf },
    VolLabel,
    McdDrive,
    McdCopy { op: Op },
    McdRename,
    McdMkdir,
    McdDelete { left: u8 },
}

/// What the screen shows besides the file list.
pub enum Mode {
    Main,
    Mcd(Box<Mcd>),
    Menu { m: usize, i: usize },
    Help { lines: Vec<String>, top: usize },
    Saver { tick: u32 },
    Attr { names: Vec<String>, a: fsx::Attr },
}

pub struct App {
    pub cfg: Config,
    pub dir: PathBuf,
    pub prev_dir: Option<PathBuf>,
    pub work: Option<PathBuf>,
    pub drive_dirs: HashMap<char, PathBuf>,
    pub drives: Vec<Drive>,
    pub entries: Vec<Entry>,
    pub cur: usize,
    pub filter: Vec<String>,
    pub sort: Sort,
    pub desc: bool,
    pub cols_mode: u8,
    pub lower: bool,
    pub show_attr: bool,
    pub hidden: bool,
    pub exe_only: bool,
    pub auto_line: bool,
    pub hline: bool,
    pub lines: Option<u16>,
    pub descs: Descs,
    pub search: Option<String>,
    pub modal: Option<(Modal, Pending)>,
    pub stash: Vec<(Modal, Pending)>,
    pub mode: Mode,
    pub arc: Option<ArcView>,
    pub job: Option<Job>,
    pub hint: Option<bool>,
    pub msg: Option<String>,
    pub tag_seq: u32,
    pub idle: Instant,
    pub history: Vec<String>,
    pub space: Option<(u64, u64)>,
    pub area_h: u16,
    pub area_w: u16,
    pub goto_file: Option<PathBuf>,
    pub quit_mcd: bool,
    pub stash_def: Option<String>,
}

pub fn his_path() -> PathBuf {
    mdir_core::term::data_dir().join("M.HIS")
}

impl App {
    pub fn new(cfg: Config, dir: PathBuf) -> App {
        cfg.apply_language();
        let history = std::fs::read_to_string(his_path()).map(|s| s.lines().map(|l| l.to_string()).collect()).unwrap_or_default();
        let mut a = App {
            sort: Sort::from_index(cfg.num("SORT")),
            desc: cfg.flag("SORTDESC"),
            cols_mode: cfg.num("COLUMN").clamp(0, 6) as u8,
            lower: cfg.flag("LOWER"),
            show_attr: cfg.flag("ATTR"),
            hidden: cfg.flag("HIDDEN") || cfg.flag("ATTR"),
            exe_only: cfg.flag("EXEONLY"),
            auto_line: cfg.flag("AUTOLINE"),
            hline: cfg.flag("HORILINE"),
            lines: match cfg.num("LINES") { 1 => Some(25), 2 => Some(28), 3 => Some(50), _ => None },
            cfg,
            dir: PathBuf::new(),
            prev_dir: None,
            work: None,
            drive_dirs: HashMap::new(),
            drives: fsx::drives(),
            entries: vec![],
            cur: 0,
            filter: vec![],
            descs: Descs::default(),
            search: None,
            modal: None,
            stash: vec![],
            mode: Mode::Main,
            arc: None,
            job: None,
            hint: None,
            msg: None,
            tag_seq: 0,
            idle: Instant::now(),
            history,
            space: None,
            area_h: 25,
            area_w: 80,
            goto_file: None,
            quit_mcd: false,
            stash_def: None,
        };
        a.chdir(dir);
        a
    }

    // ---- directory -------------------------------------------------------------------------

    pub fn reload(&mut self) {
        if let Some(av) = &self.arc {
            self.entries = av.entries(self.cfg.flag("DIREMUL"));
            crate::list::sort(&mut self.entries, self.sort, self.desc, &self.cfg.colors);
            self.cur = self.cur.min(self.entries.len().saturating_sub(1));
            return;
        }
        let keep: Option<String> = self.entries.get(self.cur).map(|e| e.name.clone());
        let tags: HashMap<String, u32> = self.entries.iter().filter(|e| e.tag > 0).map(|e| (e.name.clone(), e.tag)).collect();
        let o = ListOpts { hidden: self.hidden, exe_only: self.exe_only, filter: &self.filter };
        self.entries = list::read_dir(&self.dir, &o, &self.drives).unwrap_or_default();
        for e in &mut self.entries {
            if let Some(t) = tags.get(&e.name) {
                e.tag = *t;
            }
        }
        list::sort(&mut self.entries, self.sort, self.desc, &self.cfg.colors);
        self.descs = Descs::load(&self.dir);
        self.space = fsx::disk_space(&self.dir);
        if let Some(n) = keep {
            if let Some(i) = self.entries.iter().position(|e| e.name == n) {
                self.cur = i;
            }
        }
        self.cur = self.cur.min(self.entries.len().saturating_sub(1));
    }

    pub fn chdir(&mut self, dir: PathBuf) {
        let dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        if !dir.is_dir() {
            self.msg = Some(t("디렉토리가 존재하지 않습니다", "Cannot find such a directory").into());
            return;
        }
        let came_from = self.dir.clone();
        if dir != self.dir && !self.dir.as_os_str().is_empty() {
            self.prev_dir = Some(self.dir.clone());
        }
        self.dir = dir;
        let _ = std::env::set_current_dir(&self.dir);
        if let Some(d) = fsx::drive_of(&self.dir, &self.drives) {
            self.drive_dirs.insert(d.letter, self.dir.clone());
        }
        self.entries.clear();
        self.cur = 0;
        self.search = None;
        self.reload();
        // coming up from a child: put the bar on it
        if came_from.parent() == Some(self.dir.as_path()) {
            if let Some(n) = came_from.file_name().map(|n| n.to_string_lossy().to_string()) {
                if let Some(i) = self.entries.iter().position(|e| e.name == n) {
                    self.cur = i;
                }
            }
        }
    }

    pub fn cur_entry(&self) -> Option<&Entry> {
        self.entries.get(self.cur)
    }

    /// Tagged file names, or the one under the bar when nothing is tagged.
    pub fn targets(&self, files_only: bool) -> Vec<String> {
        let mut v: Vec<&Entry> = self.entries.iter().filter(|e| e.tag > 0).collect();
        v.sort_by_key(|e| e.tag);
        if v.is_empty() {
            if let Some(e) = self.cur_entry() {
                if e.is_file() || (!files_only && e.kind == Kind::Dir) {
                    return vec![e.name.clone()];
                }
            }
            return vec![];
        }
        v.into_iter().filter(|e| !files_only || e.is_file()).map(|e| e.name.clone()).collect()
    }

    pub fn fmt(&self) -> Fmt {
        Fmt { lower: self.lower, attr: self.show_attr, h24: self.cfg.flag("24HOUR"), drive_type: self.cfg.flag("DRIVETYPE") }
    }

    pub fn list_rows(&self) -> usize {
        // rows - 6 with the two lines, rows - 4 without them (Alt-\ code at M.EXE 0xb6e4)
        (self.area_h as usize).saturating_sub(if self.hline { 6 } else { 4 }).max(1)
    }

    /// Columns on screen: the chosen mode, or automatic (mode 1 when descriptions are on screen).
    pub fn columns(&self) -> usize {
        if self.show_attr && self.cols_mode > 2 {
            return 2;
        }
        if self.cols_mode > 0 {
            return self.cols_mode as usize;
        }
        if self.cfg.num("DESCVIEW") == 1 && !self.descs.map.is_empty() && self.arc.is_none() {
            return 1;
        }
        let rows = self.list_rows();
        let max = if self.cfg.flag("COLUMN56") { 6 } else { 4 };
        let max = if self.show_attr { 2 } else { max };
        (2..=max).find(|n| n * rows >= self.entries.len()).unwrap_or(max)
    }

    pub fn page_size(&self) -> usize {
        self.list_rows() * self.columns()
    }

    // ---- running ---------------------------------------------------------------------------

    fn var_ctx(&self) -> (Option<String>, Option<String>) {
        match self.cur_entry() {
            Some(e) if e.kind == Kind::Drive => (None, Some(e.drive.as_ref().map(|d| d.root.to_string_lossy().to_string()).unwrap_or_default())),
            Some(e) if e.kind == Kind::Up => (Some("..".into()), None),
            Some(e) => (Some(e.name.clone()), None),
            None => (None, None),
        }
    }

    /// Expands a program definition and runs it (or asks which one / lets it be edited first).
    pub fn run_def(&mut self, def: &str, quit: bool, edit: bool) -> Flow {
        let alts = alternatives(def);
        if alts.is_empty() {
            return Flow::Cont;
        }
        if alts.len() > 1 {
            let m = Modal::menu(t("프로그램을 선택하세요", "Select a program"), alts.iter().map(|a| format!(" {} ", a)).collect(), 0);
            self.modal = Some((m, Pending::ProgChoice { alts, quit, edit }));
            return Flow::Cont;
        }
        self.run_one(&alts[0], quit, edit, false)
    }

    pub fn run_one(&mut self, def: &str, quit: bool, edit: bool, dos: bool) -> Flow {
        let (file, drive) = self.var_ctx();
        let mdir = mdir_core::term::exe_dir();
        let ctx = VarCtx { dir: &self.dir, file: file.as_deref(), drive: drive.as_deref(), mdir: &mdir, qcd: &self.cfg.qcd, cfg: &self.cfg };
        let ex = match expand(def, &ctx) {
            Ok(e) => e,
            Err(e) => {
                self.error(&e);
                return Flow::Cont;
            }
        };
        let pause = ex.pause || if dos { self.cfg.flag("PAUSE-DOSCMD") } else { self.cfg.flag("PAUSE") };
        if edit || ex.prompt.is_some() {
            let mut m = Modal::input(t("수정", "Edit"), &[t("명령을 확인하거나 수정하세요", "Check or edit the command")], &ex.cmd);
            if let (Modal::Input { edit: le, w, .. }, Some(p)) = (&mut m, ex.prompt) {
                le.fresh = false;
                le.cur = p.min(le.text.len());
                *w = 64;
            }
            self.modal = Some((m, Pending::EditCmd { quit, pause }));
            return Flow::Cont;
        }
        let mut cwd = self.dir.clone();
        if ex.in_prog_dir {
            if let Some(p) = ex.cmd.split_whitespace().next().and_then(|p| Path::new(p).parent().map(|d| d.to_path_buf())) {
                if p.is_dir() {
                    cwd = p;
                }
            }
        }
        self.launch(ex.cmd, cwd, pause, quit)
    }

    pub fn launch(&mut self, cmd: String, cwd: PathBuf, pause: bool, quit: bool) -> Flow {
        let goto = std::env::temp_dir().join(format!("mdir-goto-{}", std::process::id()));
        let _ = std::fs::remove_file(&goto);
        std::env::set_var("MDIR_GOTO", &goto);
        self.goto_file = Some(goto);
        Flow::Run(RunReq { cmd, cwd, pause, quit_after: quit })
    }

    /// Enter / Tab on the bar.
    fn run_cur(&mut self, tab: bool, param: bool, quit: bool) -> Flow {
        let Some(e) = self.cur_entry().cloned() else { return Flow::Cont };
        if self.arc.is_some() {
            return self.arc_enter(&e, tab);
        }
        match e.kind {
            Kind::Up => {
                self.parent();
                return Flow::Cont;
            }
            Kind::Dir => {
                self.chdir(self.dir.join(&e.name));
                return Flow::Cont;
            }
            Kind::Drive => {
                self.change_drive(e.drive.clone().unwrap());
                return Flow::Cont;
            }
            _ => {}
        }
        if !tab && !param && !quit && crate::arc::is_archive(&self.dir.join(&e.name)) {
            self.open_archive(&e.name);
            return Flow::Cont;
        }
        if let Some(def) = self.cfg.ext_prog(e.ext()).map(|s| s.to_string()) {
            if param {
                let m = Modal::input(t("파라미터", "Parameters"), &[&def], "");
                self.modal = Some((m, Pending::Param { quit }));
                self.stash_def = Some(def);
                return Flow::Cont;
            }
            return self.run_def(&def, quit, false);
        }
        if e.exec || param || quit {
            let prog = if cfg!(windows) { e.name.clone() } else { format!("./{}", e.name) };
            let prog = if prog.contains(' ') { format!("\"{}\"", prog) } else { prog };
            if param {
                let m = Modal::input(t("파라미터", "Parameters"), &[&e.name], "");
                self.modal = Some((m, Pending::Param { quit }));
                self.stash_def = Some(if e.exec { prog } else { e.name.clone() });
                return Flow::Cont;
            }
            if !e.exec {
                return Flow::Cont;
            }
            let pause = self.cfg.flag("PAUSE");
            return self.launch(prog, self.dir.clone(), pause, quit);
        }
        Flow::Cont
    }

    pub fn change_drive(&mut self, d: Drive) {
        let target = self.drive_dirs.get(&d.letter).cloned().filter(|p| p.is_dir()).unwrap_or(d.root.clone());
        self.arc = None;
        self.chdir(target);
    }

    pub fn parent(&mut self) {
        if let Some(p) = self.dir.parent().map(|p| p.to_path_buf()) {
            self.chdir(p);
        }
    }

    pub fn error(&mut self, msg: &str) {
        self.modal = Some((Modal::msg(t("에러", "Error"), &[msg]), Pending::Info));
    }

    pub fn info(&mut self, title: &str, lines: &[&str]) {
        self.modal = Some((Modal::msg(title, lines), Pending::Info));
    }

    fn save_history(&mut self, cmd: &str) {
        self.history.retain(|h| h != cmd);
        self.history.insert(0, cmd.to_string());
        self.history.truncate(20);
        let p = his_path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(p, self.history.join("\n"));
    }

    // ---- keys ------------------------------------------------------------------------------

    pub fn key(&mut self, k: KeyEvent) -> Flow {
        self.idle = Instant::now();
        if let KeyCode::Modifier(m) = k.code {
            use mdir_core::ratatui::crossterm::event::ModifierKeyCode as M;
            self.hint = match m {
                M::LeftAlt | M::RightAlt => Some(false),
                M::LeftControl | M::RightControl => Some(true),
                _ => None,
            };
            if k.kind == mdir_core::ratatui::crossterm::event::KeyEventKind::Release {
                self.hint = None;
            }
            return Flow::Cont;
        }
        if k.kind == mdir_core::ratatui::crossterm::event::KeyEventKind::Release {
            return Flow::Cont;
        }
        self.hint = None;
        self.msg = None;
        if let Some((mut m, p)) = self.modal.take() {
            let r = m.key(k);
            return self.modal_result(m, p, r);
        }
        match std::mem::replace(&mut self.mode, Mode::Main) {
            Mode::Main => {}
            Mode::Mcd(mut mcd) => {
                let r = mcd.key(k, self);
                return self.mcd_result(mcd, r);
            }
            Mode::Menu { m, i } => return self.menu_key(k, m, i),
            Mode::Help { lines, top } => {
                let rows = self.area_h.saturating_sub(4) as usize;
                let max = lines.len().saturating_sub(rows);
                let top = match k.code {
                    KeyCode::Up => top.saturating_sub(1),
                    KeyCode::Down => (top + 1).min(max),
                    KeyCode::PageUp => top.saturating_sub(rows),
                    KeyCode::PageDown | KeyCode::Char(' ') => (top + rows).min(max),
                    KeyCode::Home => 0,
                    KeyCode::End => max,
                    _ => return Flow::Cont,
                };
                self.mode = Mode::Help { lines, top };
                return Flow::Cont;
            }
            Mode::Saver { .. } => return Flow::Cont,
            Mode::Attr { names, a } => return self.attr_key(k, names, a),
        }
        self.main_key(k)
    }

    pub fn stash_def_take(&mut self) -> String {
        self.stash_def.take().unwrap_or_default()
    }

    fn main_key(&mut self, k: KeyEvent) -> Flow {
        // speed search editing
        if let Some(s) = self.search.clone() {
            match k.code {
                KeyCode::Esc => {
                    self.search = None;
                    return Flow::Cont;
                }
                KeyCode::Backspace if !ctrl(&k) => {
                    let mut s = s;
                    s.pop();
                    self.search = if s.is_empty() { None } else { Some(s) };
                    if let Some(s) = self.search.clone() {
                        self.find(&s, 0);
                    }
                    return Flow::Cont;
                }
                KeyCode::Char('l') if ctrl(&k) => {
                    let from = self.cur + 1;
                    self.find(&s, from);
                    return Flow::Cont;
                }
                _ => {}
            }
        }
        let rows = self.list_rows();
        let n = self.entries.len();
        let last = n.saturating_sub(1);
        let c = ctrl(&k);
        match k.code {
            KeyCode::Up => self.cur = self.cur.saturating_sub(1),
            KeyCode::Down => self.cur = (self.cur + 1).min(last),
            KeyCode::Left if c => self.cur = self.prev_rowwise(),
            KeyCode::Right if c => self.cur = self.next_rowwise(),
            KeyCode::Left => self.cur = self.cur.saturating_sub(rows),
            KeyCode::Right => self.cur = (self.cur + rows).min(last),
            KeyCode::Home if c => self.cur = self.entries.iter().position(|e| e.is_file()).unwrap_or(0),
            KeyCode::End if c => self.cur = self.entries.iter().rposition(|e| e.is_file()).unwrap_or(last),
            KeyCode::Home => self.cur = 0,
            KeyCode::End => self.cur = last,
            KeyCode::PageUp if c => self.cur = self.entries.iter().position(|e| e.is_dirlike()).unwrap_or(0),
            KeyCode::PageDown if c => self.cur = self.entries.iter().rposition(|e| e.is_dirlike()).unwrap_or(0),
            KeyCode::PageUp => {
                let ps = self.page_size();
                let first = self.cur / ps * ps;
                self.cur = if self.cur != first { first } else { self.cur.saturating_sub(ps) };
            }
            KeyCode::PageDown => {
                let ps = self.page_size();
                let lastp = (self.cur / ps * ps + ps - 1).min(last);
                self.cur = if self.cur != lastp { lastp } else { (self.cur + ps).min(last) };
            }
            KeyCode::Char('[') => self.cur = self.entries.iter().position(|e| e.kind == Kind::Drive).unwrap_or(self.cur),
            KeyCode::Char(']') => {
                let ds: Vec<usize> = (0..n).filter(|&i| self.entries[i].kind == Kind::Drive).collect();
                if !ds.is_empty() {
                    let p = ds.iter().position(|&i| i == self.cur);
                    self.cur = match p {
                        Some(0) | None => *ds.last().unwrap(),
                        Some(p) => ds[p - 1],
                    };
                }
            }
            KeyCode::Esc => return self.esc_key(),
            KeyCode::Char('l') if c => {
                if let Some(s) = self.search.clone() {
                    let from = self.cur + 1;
                    self.find(&s, from);
                }
            }
            _ => {
                if let Some(a) = key_act(&k) {
                    return self.act(a);
                }
                if let KeyCode::Char(ch) = k.code {
                    if !c && !alt(&k) {
                        if ch.is_ascii_uppercase() && self.arc.is_none() {
                            let l = ch;
                            if let Some(d) = self.drives.iter().find(|d| d.letter == l).cloned() {
                                self.change_drive(d);
                            }
                            return Flow::Cont;
                        }
                        let mut s = self.search.clone().unwrap_or_default();
                        if self.cfg.flag("EXTSEARCH") && s.chars().count() == 1 && s.starts_with(ch) {
                            let from = self.cur + 1;
                            self.find(&s, from);
                            return Flow::Cont;
                        }
                        s.push(ch);
                        self.find(&s, 0);
                        self.search = Some(s);
                    }
                }
            }
        }
        let _ = shift;
        Flow::Cont
    }

    fn prev_rowwise(&self) -> usize {
        let rows = self.list_rows();
        let ps = self.page_size();
        let first = self.cur / ps * ps;
        let (col, row) = ((self.cur - first) / rows, (self.cur - first) % rows);
        if col > 0 {
            self.cur - rows
        } else if row > 0 {
            let cols = self.columns();
            let i = first + (cols - 1) * rows + row - 1;
            i.min(self.entries.len().saturating_sub(1))
        } else {
            self.cur
        }
    }

    fn next_rowwise(&self) -> usize {
        let rows = self.list_rows();
        let ps = self.page_size();
        let first = self.cur / ps * ps;
        let row = (self.cur - first) % rows;
        let n = self.entries.len();
        if self.cur + rows < n && self.cur + rows < first + ps {
            self.cur + rows
        } else if first + row + 1 < n && row + 1 < rows {
            first + row + 1
        } else {
            self.cur
        }
    }

    /// Speed search: first entry from `from` whose name starts with `s` (any case).
    pub fn find(&mut self, s: &str, from: usize) -> bool {
        let s = s.to_lowercase();
        let n = self.entries.len();
        for k in 0..n {
            let i = (from + k) % n.max(1);
            if self.entries[i].kind != Kind::Drive && self.entries[i].name.to_lowercase().starts_with(&s) {
                self.cur = i;
                return true;
            }
        }
        false
    }

    fn esc_key(&mut self) -> Flow {
        if self.arc.is_some() {
            self.close_archive();
            return Flow::Cont;
        }
        match self.cfg.num("ESC") {
            1 => self.act(Act::DosScreen),
            2 => Flow::Quit,
            3 => self.act(Act::DosCmd),
            4 => self.act(Act::DosCmdSwap),
            5 => self.act(Act::Menu),
            6 => self.act(Act::Mcd),
            7 => self.act(Act::Qcd),
            8 => self.act(Act::Parent),
            9 => self.act(Act::Root),
            _ => Flow::Cont,
        }
    }

    pub fn act(&mut self, a: Act) -> Flow {
        use Act::*;
        if self.arc.is_some() {
            if let Some(f) = self.arc_act(&a) {
                return f;
            }
        }
        match a {
            Quit => return Flow::Quit,
            Menu => self.mode = Mode::Menu { m: 0, i: 0 },
            Help => self.open_help(),
            About => {
                let lines = [
                    "M-dir - MdirIII 3.10 compatible file manager",
                    "https://github.com/Sudo42b/M-dir  (MIT OR Apache-2.0)",
                    "",
                    t("MdirIII 및 MDIR COMPUTER와 관계없는 독립 재구현", "Independent reimplementation, not affiliated with MdirIII"),
                ];
                self.info(t("제작번호", "About"), &lines);
            }
            ViewEnv => {
                let mut v: Vec<String> = vec![format!("M.CFG : {}", self.cfg.path.display())];
                for (k, n) in mdir_core::cfg::fkey_names().iter().map(|k| (k.clone(), self.cfg.fkeys.get(k).cloned().unwrap_or_default())) {
                    if !n.is_empty() {
                        v.push(format!("{:<8} {}", mdir_core::cfg::fkey_label(&k), n));
                    }
                }
                v.push(String::new());
                for (e, p) in &self.cfg.ext {
                    v.push(format!(".{:<7} {}", e, p));
                }
                self.mode = Mode::Help { lines: v, top: 0 };
            }
            Park => {
                if let Some(p) = self.cfg.opts.get("PARK").filter(|p| !p.is_empty()).cloned() {
                    return self.run_def(&p, false, false);
                }
                #[cfg(unix)]
                let _ = std::process::Command::new("sync").status();
                self.info(t("파킹", "Parking"), &[t("하드디스크가 파킹되었습니다", "Parking is completed"), t("컴퓨터의 전원을 내리세요", "You may turn off the power")]);
            }
            VolLabel => {
                if cfg!(windows) {
                    let m = Modal::input(t("볼륨명바꾸기", "Volume label"), &[t("새 볼륨명", "New volume label")], "");
                    self.modal = Some((m, Pending::VolLabel));
                } else {
                    self.error(t("처리할 수 없습니다", "Cannot control"));
                }
            }
            Format => return self.run_def(&self.cfg.get("FORMAT"), false, false),
            DiskCopy => return self.run_def(&self.cfg.get("DISKCOPY"), false, false),
            Setup => {
                let p = format!("%M{}", if cfg!(windows) { "mset.exe" } else { "mset" });
                return self.run_one(&p, false, false, false);
            }
            Saver => self.mode = Mode::Saver { tick: 0 },
            DosCmd | DosCmdSwap => {
                let init = self.history.first().cloned().unwrap_or_default();
                let mut m = Modal::input(t("도스명령", "Dos command"), &[&format!("{}>", self.dir.display())], &init);
                if let Modal::Input { hist, w, .. } = &mut m {
                    *hist = self.history.clone();
                    *w = 66;
                }
                self.modal = Some((m, Pending::DosCmd));
            }
            DosScreen => return Flow::Run(RunReq { cmd: String::new(), cwd: self.dir.clone(), pause: true, quit_after: false }),
            Run => return self.run_cur(false, false, false),
            RunSwap => return self.run_cur(true, false, false),
            RunParam | RunSwapParam => return self.run_cur(true, true, false),
            RunQuit => return self.run_cur(true, false, true),
            RunQuitParam => return self.run_cur(true, true, true),
            Fkey(k) => {
                if let Some(def) = self.cfg.fkeys.get(&k).cloned() {
                    return self.run_def(&def, false, false);
                }
            }
            Tag => {
                if let Some(e) = self.entries.get_mut(self.cur) {
                    if matches!(e.kind, Kind::File | Kind::Dir) {
                        if e.tag > 0 {
                            e.tag = 0;
                        } else {
                            self.tag_seq += 1;
                            e.tag = self.tag_seq;
                        }
                    }
                }
                self.cur = (self.cur + 1).min(self.entries.len().saturating_sub(1));
            }
            SelectAll => {
                let any = self.entries.iter().any(|e| e.tag > 0);
                for e in self.entries.iter_mut() {
                    e.tag = 0;
                    if !any && e.is_file() {
                        self.tag_seq += 1;
                        e.tag = self.tag_seq;
                    }
                }
            }
            Invert => {
                for e in self.entries.iter_mut().filter(|e| e.is_file()) {
                    if e.tag > 0 {
                        e.tag = 0;
                    } else {
                        self.tag_seq += 1;
                        e.tag = self.tag_seq;
                    }
                }
            }
            SelectGroup | DeselectGroup => {
                let sel = a == SelectGroup;
                let m = Modal::input(t(if sel { "선택" } else { "선택취소" }, if sel { "Select" } else { "DeSelect" }), &[t("파일명 (와일드카드 *, ?)", "File name (wildcards *, ?)")], "*.*");
                self.modal = Some((m, Pending::SelWild { select: sel }));
            }
            SelectExt => {
                if let Some(e) = self.cur_entry().filter(|e| e.is_file()).cloned() {
                    let on = e.tag == 0;
                    let ext = e.ext().to_lowercase();
                    for x in self.entries.iter_mut().filter(|x| x.is_file() && x.ext().to_lowercase() == ext) {
                        if on {
                            self.tag_seq += 1;
                            x.tag = self.tag_seq;
                        } else {
                            x.tag = 0;
                        }
                    }
                }
            }
            Copy | CopyIns => {
                let one = a == CopyIns && self.cfg.flag("INSDEL") || a == Copy && self.cfg.flag("SWAPDEL");
                self.start_copy(Op::Copy, one);
            }
            Move => self.start_copy(Op::Move, false),
            Delete | DeleteDel => {
                if a == DeleteDel && !self.cfg.flag("DELKEY") {
                    return Flow::Cont;
                }
                let one = a == DeleteDel && self.cfg.flag("INSDEL") || a == Delete && self.cfg.flag("SWAPDEL");
                self.start_delete(one);
            }
            Rename => {
                let names = self.targets(false);
                if !names.is_empty() {
                    self.ask_rename(names, 0);
                }
            }
            Mkdir => {
                let m = Modal::input(t("디렉토리 만들기", "^MAKE^ Directory"), &[t("만들 디렉토리 이름", "Directory name")], "");
                self.modal = Some((m, Pending::Mkdir));
            }
            Describe => {
                let names = self.targets(false);
                if let Some(n) = names.first() {
                    let init = self.descs.get(n).unwrap_or("").to_string();
                    let title = format!("{}{}", t("주석 입력 - ", "Edit description - "), n);
                    let mut m = Modal::input(&title, &[t("최대 80자", "Up to 80 characters")], &init);
                    if let Modal::Input { edit, .. } = &mut m {
                        edit.max = 80;
                    }
                    self.modal = Some((m, Pending::Describe { names }));
                }
            }
            LfnDesc => {
                let mut n = 0;
                for e in self.entries.iter().filter(|e| e.is_file() || e.kind == Kind::Dir) {
                    let (b, x) = mdir_core::text::split_ext(&e.name);
                    if (b.len() > 8 || x.len() > 3 || e.name.contains(' ')) && self.descs.get(&e.name).is_none() {
                        self.descs.set(&e.name, &e.name);
                        n += 1;
                    }
                }
                if n > 0 {
                    let _ = self.descs.save(&self.dir);
                }
                self.msg = Some(format!("{} {}", n, t("개의 긴 파일명을 주석으로 바꿈", "long names converted to descriptions")));
            }
            View => return self.run_def(&self.cfg.get("VIEW"), false, false),
            Find => return self.run_def(&self.cfg.get("FIND"), false, false),
            Print => {
                let files = self.targets(true);
                if let Some(f) = files.first() {
                    let cmd = if cfg!(windows) { format!("print \"{}\"", f) } else { format!("lp \"{}\"", f) };
                    return self.launch(cmd, self.dir.clone(), true, false);
                }
            }
            Attrib => {
                let names = self.targets(false);
                if let Some(n) = names.first() {
                    let a = std::fs::metadata(self.dir.join(n)).map(|m| fsx::attr(n, &m)).unwrap_or_default();
                    self.mode = Mode::Attr { names, a };
                }
            }
            Mcd => self.open_mcd(Purpose::Go),
            Qcd => self.open_qcd(false),
            Qcd1(i) => {
                let d = self.cfg.qcd[i].clone();
                if !d.is_empty() {
                    self.chdir(PathBuf::from(d));
                }
            }
            Parent => self.parent(),
            Root => {
                let root = fsx::drive_of(&self.dir, &self.drives).map(|d| d.root).unwrap_or_else(|| PathBuf::from("/"));
                self.chdir(root);
            }
            Prev => {
                if let Some(p) = self.prev_dir.clone() {
                    self.chdir(p);
                }
            }
            Work => match self.work.clone() {
                None => self.work = Some(self.dir.clone()),
                Some(w) if w == self.dir => {}
                Some(w) => {
                    self.work = Some(self.dir.clone());
                    self.chdir(w);
                }
            },
            WorkClose => self.work = None,
            ArcView => {
                if let Some(e) = self.cur_entry().filter(|e| e.is_file()).cloned() {
                    self.open_archive(&e.name);
                }
            }
            ArcAdd | ArcMove => {
                let items = self.targets(false);
                if !items.is_empty() {
                    let base = mdir_core::text::split_ext(&items[0]).0.to_string();
                    let ext = mdir_core::cfg::ARCHIVES[self.cfg.num("ARCHIVE").clamp(0, 10) as usize].0;
                    let init = format!("{}.{}", if items.len() == 1 { base } else { self.dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or("ARCHIVE".into()) }, ext.to_lowercase());
                    let title = if a == ArcMove { t("<압축한후 원본지우기>", "<MOVE>") } else { t("<압축하기>", "<ADD>") };
                    let m = Modal::input(title, &[t("만들어질 압축파일명을 입력하세요", "Type create archive filename")], &init);
                    self.modal = Some((m, Pending::ArcName { mv: a == ArcMove, items }));
                }
            }
            ArcExtract => {
                let arcs: Vec<PathBuf> = self.targets(true).into_iter().map(|n| self.dir.join(n)).filter(|p| crate::arc::is_archive(p)).collect();
                if !arcs.is_empty() {
                    let init = self.work.clone().unwrap_or(self.dir.clone()).to_string_lossy().to_string();
                    let m = Modal::input(t("압축풀기", "^EXTRACT^"), &[t("압축을 풀어 놓을 곳은?", "^EXTRACT^ file to")], &init);
                    self.modal = Some((m, Pending::ExtractDest { archives: arcs, inner: vec![] }));
                }
            }
            DirEmul => {}
            ExeOnly => {
                self.exe_only = !self.exe_only;
                self.reload();
            }
            ViewAttr => {
                self.show_attr = !self.show_attr;
                self.hidden = self.show_attr;
                self.reload();
            }
            Hidden => {
                self.hidden = !self.hidden;
                self.reload();
            }
            Filter => {
                let init = if self.filter.is_empty() { "*.*".to_string() } else { self.filter.join(";") };
                let m = Modal::input(t("파일 표시 필터", "File filter"), &[t("예: *.doc;*.txt  .gif;.pcx  c:\\", "e.g. *.doc;*.txt  .gif;.pcx  c:\\")], &init);
                self.modal = Some((m, Pending::Filter));
            }
            Lower => self.lower = !self.lower,
            Lines => {
                self.lines = match self.lines {
                    None => Some(25),
                    Some(25) => Some(28),
                    Some(28) => Some(50),
                    _ => None,
                }
            }
            AutoLine => self.auto_line = !self.auto_line,
            HLine => self.hline = !self.hline,
            GraphicChar => {
                let h = (self.cfg.num("HANGUL") + 1) % 4;
                self.cfg.set("HANGUL", &h.to_string());
                self.cfg.apply_language();
            }
            CycleColumns => self.cols_mode = (self.cols_mode + 1) % 7,
            CycleSort => {
                self.sort = Sort::from_index((self.sort.index() + 1) % 7);
                self.reload();
            }
            Columns(n) => self.cols_mode = n,
            SortBy(s) => {
                self.sort = s;
                self.reload();
            }
            SortDir => {
                self.desc = !self.desc;
                self.reload();
            }
        }
        Flow::Cont
    }

    fn open_help(&mut self) {
        // The original help files aren't shipped; use the person's own copy from the data folder if present.
        let ko = mdir_core::text::korean();
        let own = std::fs::read(mdir_core::term::data_dir().join(if ko { "M-K.HLP" } else { "M.HLP" })).ok();
        let text = match own {
            Some(b) if ko => ks_help(&b),
            Some(b) => mdir_core::text::cp437(&b),
            None => (if ko { crate::help::KO } else { crate::help::EN }).to_string(),
        };
        self.mode = Mode::Help { lines: text.lines().map(|l| l.trim_end_matches('\r').to_string()).collect(), top: 0 };
    }

    pub fn open_qcd(&mut self, insert: bool) {
        let items: Vec<String> = (0..10).map(|i| format!(" ^F{:<2}^ {:<50}", i + 1, self.cfg.qcd[i])).collect();
        let m = Modal::menu(t("디렉토리 빨리 바꾸기(QCD)", "Quick Change Directory(QCD)"), items, 0);
        self.modal = Some((m, Pending::Qcd { insert }));
    }

    pub fn open_mcd(&mut self, p: Purpose) {
        let mcd = Mcd::open(self, p);
        self.mode = Mode::Mcd(Box::new(mcd));
    }

    fn ask_rename(&mut self, names: Vec<String>, i: usize) {
        let n = names[i].clone();
        let m = Modal::input(t("<이름바꾸기>", "<RENAME>"), &[&format!("{} {}", n, t("의 새이름은", "new name"))], &n);
        self.modal = Some((m, Pending::Rename { names, i }));
    }

    // ---- modal results ---------------------------------------------------------------------

    fn modal_result(&mut self, m: Modal, p: Pending, r: MRes) -> Flow {
        match r {
            MRes::Keep => {
                self.modal = Some((m, p));
                Flow::Cont
            }
            MRes::Unhandled(k) => {
                self.modal_extra_key(m, p, k);
                Flow::Cont
            }
            MRes::Close => {
                if let Pending::Job | Pending::JobRename = p {
                    self.job = None;
                    self.reload();
                }
                if let Some(s) = self.stash.pop() {
                    self.modal = Some(s);
                }
                Flow::Cont
            }
            MRes::Choice(c) => self.choice(p, c),
            MRes::Text(s) => self.text_entered(p, s),
        }
    }

    /// Keys an input box passes on: Ins (%F), Ctrl-Ins (file name), F10 (MCD), F11 (QCD), Shift-F1..F10,
    /// PgUp/PgDn/F12 (history menu).
    fn modal_extra_key(&mut self, mut m: Modal, p: Pending, k: KeyEvent) {
        if let Modal::Input { edit, .. } = &mut m {
            match k.code {
                KeyCode::Insert if ctrl(&k) => {
                    if let Some(e) = self.cur_entry() {
                        edit.insert_str(&e.name.clone());
                    }
                }
                KeyCode::Insert => edit.insert_str("%F"),
                KeyCode::F(n) if shift(&k) && n <= 10 => {
                    let d = self.cfg.qcd[n as usize - 1].clone();
                    edit.insert_str(&d);
                }
                KeyCode::F(10) => {
                    self.stash.push((m, p));
                    self.open_mcd(Purpose::Insert);
                    return;
                }
                KeyCode::F(11) => {
                    self.stash.push((m, p));
                    self.open_qcd(true);
                    return;
                }
                KeyCode::PageUp | KeyCode::PageDown | KeyCode::F(12) if matches!(p, Pending::DosCmd) && !self.history.is_empty() => {
                    self.stash.push((m, p));
                    let items = self.history.iter().map(|h| format!(" {} ", h)).collect();
                    self.modal = Some((Modal::menu("Dos History", items, 0), Pending::History));
                    return;
                }
                _ => {}
            }
        }
        if let (Modal::Menu { sel, .. }, Pending::Qcd { .. }) = (&m, &p) {
            match k.code {
                KeyCode::Delete => {
                    self.cfg.qcd[*sel] = String::new();
                    let _ = self.cfg.save();
                    let insert = matches!(p, Pending::Qcd { insert: true });
                    let s = *sel;
                    self.open_qcd(insert);
                    if let Some((Modal::Menu { sel, .. }, _)) = &mut self.modal {
                        *sel = s;
                    }
                    return;
                }
                KeyCode::Enter if ctrl(&k) => {
                    let s = *sel;
                    self.open_mcd(Purpose::QcdSet(s));
                    return;
                }
                KeyCode::F(n) if shift(&k) && n <= 10 => {
                    let r = self.choice(p, n as usize - 1);
                    let _ = r;
                    return;
                }
                _ => {}
            }
        }
        if let (Modal::Menu { sel, .. }, Pending::History) = (&m, &p) {
            if k.code == KeyCode::Enter && ctrl(&k) {
                let h = self.history[*sel].clone();
                if let Some((Modal::Input { edit, .. }, _)) = self.stash.last_mut() {
                    edit.set(&h);
                    edit.fresh = false;
                }
                self.modal = self.stash.pop();
                return;
            }
        }
        self.modal = Some((m, p));
    }

    fn choice(&mut self, p: Pending, c: usize) -> Flow {
        if matches!(p, Pending::McdDrive | Pending::McdDelete { .. }) {
            self.mcd_choice(p, c);
            return Flow::Cont;
        }
        match p {
            Pending::ProgChoice { alts, quit, edit } => return self.run_one(&alts[c].clone(), quit, edit, false),
            Pending::ConfirmDelete { items } => {
                if c == 0 {
                    self.job = Some(Job::new(Op::Delete, &self.dir, items, None));
                    return self.pump_job();
                }
            }
            Pending::Job => return self.job_answer(c),
            Pending::Qcd { insert } => {
                let d = self.cfg.qcd[c].clone();
                if d.is_empty() {
                    self.open_mcd(Purpose::QcdSet(c));
                } else if insert {
                    if let Some((Modal::Input { edit, .. }, _)) = self.stash.last_mut() {
                        edit.insert_str(&d);
                    }
                    self.modal = self.stash.pop();
                } else if let Mode::Mcd(m) = &mut self.mode {
                    m.goto_path(Path::new(&d));
                } else {
                    self.chdir(PathBuf::from(d));
                }
            }
            Pending::History => {
                let h = self.history[c].clone();
                self.stash.pop();
                return self.text_entered(Pending::DosCmd, h);
            }
            _ => {
                if let Some(s) = self.stash.pop() {
                    self.modal = Some(s);
                }
            }
        }
        Flow::Cont
    }

    fn text_entered(&mut self, p: Pending, s: String) -> Flow {
        if matches!(p, Pending::McdMkdir | Pending::McdRename | Pending::McdCopy { .. }) {
            self.mcd_text(&p, &s);
            return Flow::Cont;
        }
        match p {
            Pending::DosCmd => {
                let s = s.trim().to_string();
                if s.is_empty() {
                    return Flow::Cont;
                }
                self.save_history(&s);
                let (cmd, pause) = match s.strip_suffix('%') {
                    Some(c) if !c.ends_with('%') => (c.to_string(), true),
                    _ => (s.clone(), false),
                };
                if let Some(rest) = cmd.trim().strip_prefix("cd ").or_else(|| cmd.trim().strip_prefix("CD ")) {
                    let p = self.dir.join(rest.trim().trim_matches('"'));
                    self.chdir(p);
                    return Flow::Cont;
                }
                let f = self.run_one(&cmd, false, false, true);
                if let Flow::Run(mut r) = f {
                    r.pause |= pause;
                    return Flow::Run(r);
                }
                return f;
            }
            Pending::Param { quit } => {
                let def = self.stash_def_take();
                let full = format!("{} {}", def, s);
                return self.run_one(full.trim(), quit, false, false);
            }
            Pending::EditCmd { quit, pause } => {
                if !s.trim().is_empty() {
                    return self.launch(s, self.dir.clone(), pause, quit);
                }
            }
            Pending::Filter => {
                let s = s.trim().to_string();
                let p = Path::new(&s);
                if !s.is_empty() && (p.is_absolute() || s.contains(std::path::MAIN_SEPARATOR)) && self.dir.join(p).is_dir() {
                    self.filter.clear();
                    self.chdir(self.dir.join(p));
                    return Flow::Cont;
                }
                self.filter = s.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty() && x != "*.*" && x != "*").take(15).collect();
                self.reload();
            }
            Pending::Mkdir => {
                let s = s.trim();
                if !s.is_empty() {
                    match std::fs::create_dir_all(self.dir.join(s)) {
                        Ok(_) => {
                            self.reload();
                            let first = s.split(['/', '\\']).next().unwrap_or(s).to_string();
                            if let Some(i) = self.entries.iter().position(|e| e.name == first) {
                                self.cur = i;
                            }
                        }
                        Err(_) => self.error(t("디렉토리를 만들 수 없습니다", "Unable to creat directory")),
                    }
                }
            }
            Pending::Rename { names, i } => {
                let old = names[i].clone();
                let new = s.trim().to_string();
                let multi = names.len() > 1;
                if !new.is_empty() && new != old {
                    let target = if new.contains('*') || new.contains('?') { wild_rename(&old, &new) } else { new.clone() };
                    let reserved = ["COM1", "COM2", "COM3", "COM4", "NUL", "PRN", "LPT1", "LPT2", "CON", "AUX"];
                    if reserved.iter().any(|r| r.eq_ignore_ascii_case(mdir_core::text::split_ext(&target).0)) || self.dir.join(&target).exists() {
                        self.error(t("이름이 잘못되었거나 이미 존재합니다", "Already exist or Bad file name"));
                        return Flow::Cont;
                    }
                    if std::fs::rename(self.dir.join(&old), self.dir.join(&target)).is_ok() {
                        fsx::carry_desc(&self.dir, &old, &self.dir, &target, true);
                        if let Some(e) = self.entries.iter_mut().find(|e| e.name == old) {
                            e.name = target.clone();
                        }
                    } else {
                        self.error(t("이름이 잘못되었거나 이미 존재합니다", "Already exist or Bad file name"));
                        return Flow::Cont;
                    }
                    // wildcards apply to every tagged file at once
                    if new.contains('*') || new.contains('?') {
                        for o in names.iter().skip(i + 1) {
                            let tg = wild_rename(o, &new);
                            if std::fs::rename(self.dir.join(o), self.dir.join(&tg)).is_ok() {
                                fsx::carry_desc(&self.dir, o, &self.dir, &tg, true);
                                if let Some(e) = self.entries.iter_mut().find(|e| &e.name == o) {
                                    e.name = tg;
                                }
                            }
                        }
                        self.reload();
                        return Flow::Cont;
                    }
                }
                if multi && i + 1 < names.len() {
                    self.ask_rename(names, i + 1);
                } else {
                    self.reload();
                }
            }
            Pending::Describe { names } => {
                for n in &names {
                    self.descs.set(n, &s);
                }
                if self.descs.save(&self.dir).is_err() {
                    self.error(t("처리할 수 없습니다", "Cannot control"));
                }
            }
            Pending::SelWild { select } => {
                let pats: Vec<String> = s.split(';').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                for e in self.entries.iter_mut().filter(|e| e.is_file()) {
                    if pats.iter().any(|p| wild(p, &e.name)) {
                        if select && e.tag == 0 {
                            self.tag_seq += 1;
                            e.tag = self.tag_seq;
                        } else if !select {
                            e.tag = 0;
                        }
                    }
                }
            }
            Pending::Dest { op, items } => {
                let dest = self.dir.join(s.trim());
                self.job = Some(Job::new(op, &self.dir, items, Some(dest)));
                return self.pump_job();
            }
            Pending::ArcName { mv, items } => return self.create_archive(s.trim(), items, mv),
            Pending::ExtractDest { archives, inner } => {
                let dest = self.dir.join(s.trim());
                if inner.is_empty() {
                    let m = Modal::input(t("<압축풀기>", "<EXTRACT>"), &[t("파일스펙을 입력하세요(전부:*.*)", "Type FileSpec")], "*.*");
                    self.modal = Some((m, Pending::ExtractSpec { archives, dest }));
                } else if let Some(c) = self.extract(&archives[0], &dest, &inner, "*.*") {
                    return self.launch(c, dest, true, false);
                }
            }
            Pending::ExtractSpec { archives, dest } => {
                let cmds: Vec<String> = archives.iter().filter_map(|a| self.extract(a, &dest, &[], s.trim())).collect();
                self.reload();
                if !cmds.is_empty() {
                    return self.launch(cmds.join(" && "), dest, true, false);
                }
            }
            Pending::JobRename => return self.job_rename(s),
            Pending::VolLabel => {
                let d = self.dir.to_string_lossy().chars().take(2).collect::<String>();
                return self.launch(format!("label {} {}", d, s), self.dir.clone(), false, false);
            }
            _ => {}
        }
        Flow::Cont
    }

    fn start_copy(&mut self, op: Op, one: bool) {
        let items = if one { self.cur_entry().filter(|e| matches!(e.kind, Kind::File | Kind::Dir)).map(|e| vec![e.name.clone()]).unwrap_or_default() } else { self.targets(false) };
        if items.is_empty() {
            return;
        }
        let what = if op == Op::Copy { t("복사", "COPY") } else { t("이동", "MOVE") };
        let init = self.work.clone().filter(|w| *w != self.dir).unwrap_or(self.dir.clone());
        let prompt = if mdir_core::text::korean() { format!("파일을 ^{}^할 디렉토리를 선택하세요", what) } else { format!("^{}^ file to", what) };
        let mut m = Modal::input(&format!("<{}>", what), &[&prompt, t("^F10^=MCD  ^F11^=QCD", "^F10^=MCD  ^F11^=QCD")], &init.to_string_lossy());
        if let Modal::Input { w, .. } = &mut m {
            *w = 60;
        }
        self.modal = Some((m, Pending::Dest { op, items }));
    }

    fn start_delete(&mut self, one: bool) {
        let items = if one { self.cur_entry().filter(|e| matches!(e.kind, Kind::File | Kind::Dir)).map(|e| vec![e.name.clone()]).unwrap_or_default() } else { self.targets(false) };
        if items.is_empty() {
            return;
        }
        let yes = self.cfg.flag("DEL_YES");
        let line = if items.len() == 1 { items[0].clone() } else { format!("{} {}", items.len(), t("개 파일", "files")) };
        let m = Modal::yes_no(t("파일지우기", "DELETE"), &[t("선택된 파일을 전부 지울까요?", "DELETE Selected files"), &line], yes);
        self.modal = Some((m, Pending::ConfirmDelete { items }));
    }

    pub fn after_run(&mut self) {
        if let Some(g) = self.goto_file.take() {
            if let Ok(target) = std::fs::read_to_string(&g) {
                let p = PathBuf::from(target.trim());
                let _ = std::fs::remove_file(&g);
                if let (Some(d), Some(n)) = (p.parent(), p.file_name()) {
                    self.chdir(d.to_path_buf());
                    let n = n.to_string_lossy().to_string();
                    if let Some(i) = self.entries.iter().position(|e| e.name == n) {
                        self.cur = i;
                    }
                    return;
                }
            }
        }
        let path = self.cfg.path.clone();
        self.cfg = Config::load_from(&path);
        self.cfg.apply_language();
        self.reload();
    }

    pub fn tick(&mut self) -> Flow {
        if let Mode::Saver { tick } = &mut self.mode {
            *tick += 1;
            return Flow::Cont;
        }
        let mins = self.cfg.num("SAVETIME");
        if mins > 0 && self.idle.elapsed().as_secs() >= mins as u64 * 60 && self.modal.is_none() {
            self.mode = Mode::Saver { tick: 0 };
        }
        Flow::Cont
    }

    fn menu_key(&mut self, k: KeyEvent, m: usize, i: usize) -> Flow {
        let ms: Vec<MenuDef> = menus();
        let n = ms[m].items.len();
        let (mut m2, mut i2) = (m, i);
        match k.code {
            KeyCode::Esc => return Flow::Cont,
            KeyCode::Left => {
                m2 = (m + ms.len() - 1) % ms.len();
                i2 = 0
            }
            KeyCode::Right => {
                m2 = (m + 1) % ms.len();
                i2 = 0
            }
            KeyCode::Up => i2 = (i + n - 1) % n,
            KeyCode::Down => i2 = (i + 1) % n,
            KeyCode::Home => i2 = 0,
            KeyCode::End => i2 = n - 1,
            KeyCode::Enter => {
                let a = ms[m].items[i].act.clone();
                return self.act(a);
            }
            KeyCode::Char(c) => {
                let c = c.to_ascii_uppercase();
                if let Some(p) = "MRFDAO".find(c) {
                    m2 = p;
                    i2 = 0;
                }
            }
            _ => {}
        }
        self.mode = Mode::Menu { m: m2, i: i2 };
        Flow::Cont
    }

    fn attr_key(&mut self, k: KeyEvent, names: Vec<String>, mut a: fsx::Attr) -> Flow {
        match k.code {
            KeyCode::Esc => return Flow::Cont,
            KeyCode::Char(c) => match c.to_ascii_uppercase() {
                'A' => a.archive = !a.archive,
                'R' => a.readonly = !a.readonly,
                'H' => a.hidden = !a.hidden,
                'S' => a.system = !a.system,
                _ => {}
            },
            KeyCode::Enter | KeyCode::End => {
                let all = k.code == KeyCode::End;
                let todo: Vec<String> = if all { names.clone() } else { names[..1].to_vec() };
                let mut failed = false;
                for n in &todo {
                    failed |= crate::ops::set_attr(&self.dir.join(n), a).is_err();
                }
                let rest: Vec<String> = names.into_iter().skip(todo.len()).collect();
                self.reload();
                if failed {
                    self.error(t("속성바꾸기 실패", "Attributes Change Failure"));
                }
                if let Some(n) = rest.first() {
                    let a2 = std::fs::metadata(self.dir.join(n)).map(|m| fsx::attr(n, &m)).unwrap_or_default();
                    self.mode = Mode::Attr { names: rest, a: a2 };
                }
                return Flow::Cont;
            }
            _ => {}
        }
        self.mode = Mode::Attr { names, a };
        Flow::Cont
    }
}

/// Applies a DOS rename mask ("*.bak", "a?c.*") to a name.
pub fn wild_rename(name: &str, mask: &str) -> String {
    let (nb, ne) = mdir_core::text::split_ext(name);
    let (mb, me) = mdir_core::text::split_ext(mask);
    let part = |src: &str, m: &str| -> String {
        let s: Vec<char> = src.chars().collect();
        let mut out = String::new();
        for (i, c) in m.chars().enumerate() {
            match c {
                '*' => {
                    out.extend(s.iter().skip(i));
                    break;
                }
                '?' => {
                    if let Some(ch) = s.get(i) {
                        out.push(*ch)
                    }
                }
                _ => out.push(c),
            }
        }
        out
    };
    let b = part(nb, mb);
    let e = part(ne, me);
    if e.is_empty() {
        b
    } else {
        format!("{}.{}", b, e)
    }
}

/// M-K.HLP is KS text where every line-drawing character takes two columns.
pub fn ks_help(bytes: &[u8]) -> String {
    mdir_core::text::widen(&mdir_core::text::decode_as(bytes, mdir_core::text::Code::Ks))
}

impl mdir_core::term::Tui for App {
    fn draw(&mut self, b: &mut mdir_core::ratatui::buffer::Buffer) {
        crate::draw::draw(self, b)
    }
    fn key(&mut self, k: KeyEvent) -> Flow {
        App::key(self, k)
    }
    fn tick(&mut self) -> Flow {
        App::tick(self)
    }
    fn area(&self, full: mdir_core::ratatui::layout::Rect) -> mdir_core::ratatui::layout::Rect {
        let mut a = full;
        if !self.auto_line {
            a.width = a.width.min(80);
            a.height = a.height.min(25);
        }
        if let Some(h) = self.lines {
            a.height = a.height.min(h);
        }
        a
    }
    fn after_run(&mut self) {
        App::after_run(self)
    }
}
