//! Copy, move and delete of tagged files and directories, with MdirIII's prompts:
//! OverWrite / All / Skip_all / Skip / Rename / Newer, special attributes, non-empty and root
//! directories, "not enough free space", "failed - continue?".
use crate::app::{App, Pending};
use mdir_core::fsx::{self, Attr};
use mdir_core::modal::Modal;
use mdir_core::term::Flow;
use mdir_core::text::{commas, fmt_date, fmt_time, t};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Copy,
    Move,
    Delete,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Over {
    Ask,
    All,
    SkipAll,
    Newer,
}

#[derive(Clone, Debug)]
pub enum Wait {
    Overwrite(String),
    Special,
    NonEmpty,
    Space,
    Failed,
    RenameTo,
}

pub struct Job {
    pub op: Op,
    pub src: PathBuf,
    pub items: Vec<String>,
    pub dest: Option<PathBuf>,
    pub idx: usize,
    pub over: Over,
    pub wait: Option<Wait>,
    pub done: Vec<String>,
    pub force_one: bool,
    pub skip_one: bool,
    pub rename: Option<String>,
    pub del_all: bool,
    pub space_ok: bool,
}

impl Job {
    pub fn new(op: Op, src: &Path, items: Vec<String>, dest: Option<PathBuf>) -> Job {
        Job { op, src: src.to_path_buf(), items, dest, idx: 0, over: Over::Ask, wait: None, done: vec![], force_one: false, skip_one: false, rename: None, del_all: false, space_ok: false }
    }
}

fn tree_size(p: &Path) -> u64 {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p).map(|r| r.flatten().map(|e| tree_size(&e.path())).sum()).unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

pub fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    let m = std::fs::symlink_metadata(from)?;
    if m.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            copy_tree(&e.path(), &to.join(e.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to)?;
        if let Ok(t) = m.modified() {
            if let Ok(f) = std::fs::File::options().write(true).open(to) {
                let _ = f.set_modified(t);
            }
        }
        Ok(())
    }
}

pub fn move_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    copy_tree(from, to)?;
    if from.is_dir() {
        std::fs::remove_dir_all(from)
    } else {
        std::fs::remove_file(from)
    }
}

pub fn remove(p: &Path) -> std::io::Result<()> {
    let m = std::fs::symlink_metadata(p)?;
    if m.is_dir() {
        std::fs::remove_dir_all(p)
    } else {
        if m.permissions().readonly() {
            let mut pm = m.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            pm.set_readonly(false);
            let _ = std::fs::set_permissions(p, pm);
        }
        std::fs::remove_file(p)
    }
}

pub fn set_attr(p: &Path, a: Attr) -> std::io::Result<()> {
    let m = std::fs::metadata(p)?;
    let mut pm = m.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    pm.set_readonly(a.readonly);
    std::fs::set_permissions(p, pm)?;
    if cfg!(windows) {
        let flag = |on: bool, c: char| format!("{}{}", if on { '+' } else { '-' }, c);
        let _ = std::process::Command::new("attrib").args([flag(a.archive, 'A'), flag(a.hidden, 'H'), flag(a.system, 'S')]).arg(p).status();
    }
    Ok(())
}

fn info_line(label: &str, p: &Path, h24: bool) -> String {
    match std::fs::metadata(p) {
        Ok(m) => {
            let tm = m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            format!("{} {:>13} {} {} {}", label, commas(m.len()), fmt_date(tm), fmt_time(tm, h24), p.parent().map(|d| d.display().to_string()).unwrap_or_default())
        }
        Err(_) => label.to_string(),
    }
}

fn newer(src: &Path, dst: &Path) -> bool {
    let mt = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (mt(src), mt(dst)) {
        (Some(a), Some(b)) => a > b,
        _ => true,
    }
}

impl App {
    /// Runs the current job until it finishes or needs an answer.
    pub fn pump_job(&mut self) -> Flow {
        let Some(mut job) = self.job.take() else { return Flow::Cont };
        let h24 = self.cfg.flag("24HOUR");
        if job.op != Op::Delete && !job.space_ok {
            job.space_ok = true;
            let dest = job.dest.clone().unwrap();
            if job.items.iter().any(|n| dest.starts_with(job.src.join(n))) {
                self.error(t("자신의 디렉토리로 트리를 복사할 수 없습니다", "Cannot COPY/MOVE tree into itself"));
                return Flow::Cont;
            }
            if !dest.is_dir() && std::fs::create_dir_all(&dest).is_err() {
                self.error(t("대상 디렉토리를 만들 수 없습니다", "Cannot creat dest-directory"));
                return Flow::Cont;
            }
            let need: u64 = job.items.iter().map(|n| tree_size(&job.src.join(n))).sum();
            let same_disk = job.op == Op::Move && fsx::drive_of(&dest, &self.drives).map(|d| d.root) == fsx::drive_of(&job.src, &self.drives).map(|d| d.root);
            if let Some((_, free)) = fsx::disk_space(&dest) {
                if need > free && !same_disk {
                    let what = if job.op == Op::Copy { "COPY" } else { "MOVE" };
                    let m = Modal::yes_no(t("공간 부족", "Not enough space"), &[&format!("{}{}{}", t("파일을 ", "Not enough free space for "), what, t("할 공간이 부족합니다. 계속할까요?", ". Continue?"))], false);
                    job.wait = Some(Wait::Space);
                    self.job = Some(job);
                    self.modal = Some((m, Pending::Job));
                    return Flow::Cont;
                }
            }
        }
        while job.idx < job.items.len() {
            let name = job.items[job.idx].clone();
            let src = job.src.join(&name);
            match job.op {
                Op::Delete => {
                    let meta = std::fs::symlink_metadata(&src).ok();
                    let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                    if !job.force_one && !job.del_all {
                        if let Some(m) = &meta {
                            let a = fsx::attr(&name, m);
                            if !is_dir && (a.readonly || a.system || (a.hidden && name != fsx::DESC_FILE)) {
                                let m = Modal::ask(t("파일지우기", "DELETE"), &[&name, t("중요한 속성이 걸린 파일입니다. 지울까요?", "Special attributes file. Delete?")], vec![t(" 예(^Y^) ", " ^Y^es ").into(), t("  전부(^A^)  ", " ^A^ll ").into(), t(" 아니오(^N^) ", " ^N^o ").into()], 2);
                                job.wait = Some(Wait::Special);
                                self.job = Some(job);
                                self.modal = Some((m, Pending::Job));
                                return Flow::Cont;
                            }
                            if is_dir && std::fs::read_dir(&src).map(|mut r| r.next().is_some()).unwrap_or(false) {
                                if !self.cfg.flag("DIRDEL") {
                                    self.error(t("빈 디렉토리가 아닙니다", "Not Empty"));
                                    job.idx += 1;
                                    continue;
                                }
                                let m = Modal::ask(t("지우기", "Delete"), &[&name, t("빈 디렉토리가 아닙니다. 지울까요?", "Not empty!  Delete?")], vec![t(" 예(^Y^) ", " ^Y^es ").into(), t("  전부(^A^)  ", " ^A^ll ").into(), t(" 아니오(^N^) ", " ^N^o ").into()], 2);
                                job.wait = Some(Wait::NonEmpty);
                                self.job = Some(job);
                                self.modal = Some((m, Pending::Job));
                                return Flow::Cont;
                            }
                        }
                    }
                    job.force_one = false;
                    if remove(&src).is_ok() {
                        let mut d = fsx::Descs::load(&job.src);
                        if d.get(&name).is_some() {
                            d.set(&name, "");
                            let _ = d.save(&job.src);
                        }
                        job.done.push(name);
                    } else {
                        let m = Modal::yes_no(t("지우기 실패", "Delete failure"), &[&name, t("실패. 나머지 파일을 계속할까요?", "Failed. continue?")], true);
                        job.idx += 1;
                        job.wait = Some(Wait::Failed);
                        self.job = Some(job);
                        self.modal = Some((m, Pending::Job));
                        return Flow::Cont;
                    }
                }
                Op::Copy | Op::Move => {
                    let dest_dir = job.dest.clone().unwrap();
                    let to_name = job.rename.take().unwrap_or(name.clone());
                    let dst = dest_dir.join(&to_name);
                    if dst == src {
                        // never copy a file onto itself; only "Rename" goes on from here
                        if job.op == Op::Copy && !job.force_one && !job.skip_one && job.over == Over::Ask {
                            // same directory: only "Rename" makes sense
                            job.wait = Some(Wait::Overwrite(name.clone()));
                            let m = self.overwrite_box(&src, &dst, h24);
                            self.job = Some(job);
                            self.modal = Some((m, Pending::Job));
                            return Flow::Cont;
                        }
                        job.force_one = false;
                        job.skip_one = false;
                        job.idx += 1;
                        continue;
                    }
                    if dst.exists() && !job.force_one {
                        let skip = job.skip_one
                            || job.over == Over::SkipAll
                            || (job.over == Over::Newer && !newer(&src, &dst));
                        job.skip_one = false;
                        if skip {
                            job.idx += 1;
                            continue;
                        }
                        if job.over == Over::Ask {
                            job.wait = Some(Wait::Overwrite(name.clone()));
                            let m = self.overwrite_box(&src, &dst, h24);
                            self.job = Some(job);
                            self.modal = Some((m, Pending::Job));
                            return Flow::Cont;
                        }
                    }
                    job.force_one = false;
                    if dst.is_dir() && src.is_file() {
                        let _ = std::fs::remove_dir_all(&dst);
                    } else if dst.is_file() && src.is_dir() {
                        let _ = std::fs::remove_file(&dst);
                    }
                    let r = if job.op == Op::Copy { copy_tree(&src, &dst) } else { move_tree(&src, &dst) };
                    match r {
                        Ok(_) => {
                            fsx::carry_desc(&job.src, &name, &dest_dir, &to_name, job.op == Op::Move);
                            job.done.push(name);
                        }
                        Err(_) => {
                            let m = Modal::yes_no(t("복사/이동 실패", "COPY/MOVE failure"), &[&name, t("실패. 나머지 파일을 계속할까요?", "Failed. continue?")], true);
                            job.idx += 1;
                            job.wait = Some(Wait::Failed);
                            self.job = Some(job);
                            self.modal = Some((m, Pending::Job));
                            return Flow::Cont;
                        }
                    }
                }
            }
            job.idx += 1;
        }
        self.finish_job(job);
        Flow::Cont
    }

    fn finish_job(&mut self, job: Job) {
        // only the files that were done lose their tag
        for e in self.entries.iter_mut() {
            if job.done.contains(&e.name) {
                e.tag = 0;
            }
        }
        if let Some(d) = &job.dest {
            if *d != self.dir {
                self.prev_dir = Some(d.clone());
            }
        }
        if self.cfg.flag("CACHEFLUSH") {
            #[cfg(unix)]
            let _ = std::process::Command::new("sync").status();
        }
        let keep = self.cur;
        self.reload();
        self.cur = keep.min(self.entries.len().saturating_sub(1));
    }

    fn overwrite_box(&self, src: &Path, dst: &Path, h24: bool) -> Modal {
        let name = dst.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let same = |p: &Path, q: &Path| {
            let a = std::fs::metadata(p).ok();
            let b = std::fs::metadata(q).ok();
            match (a, b) {
                (Some(a), Some(b)) => a.len() == b.len() && a.modified().ok() == b.modified().ok(),
                _ => false,
            }
        };
        let s1 = info_line(t("원본:", "SOURCE:"), src, h24);
        let s2 = info_line(t("대상:", "DEST  :"), dst, h24);
        let s2 = if same(src, dst) { s2 } else { format!("^{}^", s2) };
        let choices: Vec<String> = if mdir_core::text::korean() {
            ["교체(^O^)", "전부교체(^A^)", "전부통과(^K^)", "통과(^S^)", "새이름(^R^)", "새파일만(^N^)"].iter().map(|s| s.to_string()).collect()
        } else {
            ["^O^verWrite", "^A^ll", "s^K^ip_all", "^S^kip", "^R^ename", "^N^ewer"].iter().map(|s| s.to_string()).collect()
        };
        Modal::ask(&format!("{}{}", name, t("은 이미 존재합니다", " already exist")), &[&s1, &s2], choices, 0)
    }

    /// Answer to the prompt the job is waiting on.
    pub fn job_answer(&mut self, c: usize) -> Flow {
        let Some(mut job) = self.job.take() else { return Flow::Cont };
        match job.wait.take() {
            Some(Wait::Space) => {
                if c != 0 {
                    return Flow::Cont;
                }
            }
            Some(Wait::Failed) => {
                if c != 0 {
                    self.finish_job(job);
                    return Flow::Cont;
                }
            }
            Some(Wait::Special) | Some(Wait::NonEmpty) => match c {
                0 => job.force_one = true,
                1 => job.del_all = true,
                _ => job.idx += 1,
            },
            Some(Wait::Overwrite(name)) => match c {
                0 => job.force_one = true,
                1 => job.over = Over::All,
                2 => job.over = Over::SkipAll,
                3 => job.skip_one = true,
                4 => {
                    let m = Modal::input(t("새이름은", "RENAME"), &[&name], &name);
                    job.wait = Some(Wait::RenameTo);
                    self.job = Some(job);
                    self.modal = Some((m, Pending::JobRename));
                    return Flow::Cont;
                }
                _ => job.over = Over::Newer,
            },
            _ => {}
        }
        self.job = Some(job);
        self.pump_job()
    }

    pub fn job_rename(&mut self, s: String) -> Flow {
        if let Some(job) = self.job.as_mut() {
            job.wait = None;
            if !s.trim().is_empty() {
                job.rename = Some(s.trim().to_string());
            } else {
                job.skip_one = true;
            }
        }
        self.pump_job()
    }
}
