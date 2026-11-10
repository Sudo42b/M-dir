//! MF — MdirIII Format 3.10. Floppy drives are gone, so it formats floppy disk image files
//! (.IMG, FAT12) with the original's menu: Drive, Size, Mode (DOS / Quick), Verify, Direction.
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
use std::path::Path;

/// (label, total sectors, sectors/cluster, root entries, media, sectors/FAT, sectors/track)
const SIZES: [(&str, u16, u8, u16, u8, u16, u16); 4] = [
    ("360K", 720, 2, 112, 0xFD, 2, 9),
    ("720K", 1440, 2, 112, 0xF9, 3, 9),
    ("1.2M", 2400, 1, 224, 0xF9, 7, 15),
    ("1.44M", 2880, 1, 224, 0xF0, 9, 18),
];

#[derive(Clone, Copy)]
struct Geo {
    total: u16,
    spc: u8,
    root: u16,
    media: u8,
    spf: u16,
    spt: u16,
    heads: u16,
}

impl Geo {
    fn from_size(i: usize) -> Geo {
        let s = SIZES[i];
        Geo { total: s.1, spc: s.2, root: s.3, media: s.4, spf: s.5, spt: s.6, heads: 2 }
    }
    /// Geometry of an already formatted image (for quick format, whatever its size).
    fn from_boot(d: &[u8]) -> Option<Geo> {
        if d.len() < 512 || d[510] != 0x55 || d[511] != 0xAA {
            return None;
        }
        let u16at = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
        let g = Geo { total: u16at(19), spc: d[13], root: u16at(17), media: d[21], spf: u16at(22), spt: u16at(24), heads: u16at(26) };
        if u16at(11) != 512 || g.total == 0 || g.spc == 0 || g.spf == 0 || g.spt == 0 {
            return None;
        }
        Some(g)
    }
    fn tracks(&self) -> u16 {
        self.total / (self.spt * self.heads.max(1))
    }
    fn data_bytes(&self) -> u64 {
        let root_secs = (self.root as u64 * 32).div_ceil(512);
        (self.total as u64 - 1 - 2 * self.spf as u64 - root_secs) * 512
    }
}

/// Boot sector, two FATs and an empty root directory for `g`.
fn system_area(g: &Geo, serial: u32) -> Vec<u8> {
    let root_secs = (g.root as usize * 32).div_ceil(512);
    let mut d = vec![0u8; 512 * (1 + 2 * g.spf as usize + root_secs)];
    d[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
    d[3..11].copy_from_slice(b"MDIRIII ");
    d[11..13].copy_from_slice(&512u16.to_le_bytes());
    d[13] = g.spc;
    d[14..16].copy_from_slice(&1u16.to_le_bytes());
    d[16] = 2;
    d[17..19].copy_from_slice(&g.root.to_le_bytes());
    d[19..21].copy_from_slice(&g.total.to_le_bytes());
    d[21] = g.media;
    d[22..24].copy_from_slice(&g.spf.to_le_bytes());
    d[24..26].copy_from_slice(&g.spt.to_le_bytes());
    d[26..28].copy_from_slice(&g.heads.to_le_bytes());
    d[38] = 0x29;
    d[39..43].copy_from_slice(&serial.to_le_bytes());
    d[43..54].copy_from_slice(b"NO NAME    ");
    d[54..62].copy_from_slice(b"FAT12   ");
    // boot code: show the message and wait (int 10h teletype loop, then int 16h, int 19h)
    let msg = b"\r\nM-dir MF image: not a system disk.\r\nBooting from hard disk.\r\n\0";
    let code: [u8; 20] = [0xFA, 0x31, 0xC0, 0x8E, 0xD8, 0xFB, 0xBE, 0x00, 0x00, 0xAC, 0x08, 0xC0, 0x74, 0x06, 0xB4, 0x0E, 0xCD, 0x10, 0xEB, 0xF5];
    d[62..62 + code.len()].copy_from_slice(&code);
    let msg_at = 62 + code.len() + 6;
    let addr = (0x7C00 + msg_at) as u16;
    d[69..71].copy_from_slice(&addr.to_le_bytes());
    d[62 + code.len()..62 + code.len() + 6].copy_from_slice(&[0x31, 0xC0, 0xCD, 0x16, 0xCD, 0x19]);
    d[msg_at..msg_at + msg.len()].copy_from_slice(msg);
    d[510] = 0x55;
    d[511] = 0xAA;
    for f in 0..2 {
        let o = 512 * (1 + f * g.spf as usize);
        d[o..o + 3].copy_from_slice(&[g.media, 0xFF, 0xFF]);
    }
    d
}

#[derive(Clone, Copy, PartialEq)]
enum What {
    Info,
    Drive,
    Confirm,
}

struct Mf {
    drive: String,
    size: usize,
    quick: bool,
    verify: bool,
    forward: bool,
    sel: usize,
    modal: Option<(Modal, What)>,
    progress: Option<String>,
    colors: Colors,
}

const MENU_N: usize = 9;

impl Mf {
    fn menu(&self) -> Vec<String> {
        if mdir_core::text::korean() {
            vec![" 포맷             ^Ins^ ".into(), "-".into(), " 드라이브 선택     ^F3^ ".into(), " 포맷 용량         ^F4^ ".into(), " 포맷 방식         ^F5^ ".into(), "-".into(), " 검사              ^F6^ ".into(), " 포맷 방향         ^F7^ ".into(), " 종료              ^@X^ ".into()]
        } else {
            vec![" Format           ^Ins^ ".into(), "-".into(), " Drive             ^F3^ ".into(), " Size              ^F4^ ".into(), " Mode              ^F5^ ".into(), "-".into(), " Verify            ^F6^ ".into(), " Direction         ^F7^ ".into(), " Quit              ^@X^ ".into()]
        }
    }

    fn info(&self) -> Vec<String> {
        let onoff = |b: bool| if b { t("켜짐(o)", "ON") } else { t("꺼짐(x)", "OFF") };
        let mode = if self.quick { t("빠른포맷", "Quick ") } else { t("도스포맷", "DOS   ") };
        let dir = if self.forward { t("앞으로", "Forward ") } else { t("뒤로  ", "Backward") };
        if mdir_core::text::korean() {
            vec![format!(" 디스크 - ^{}^", self.drive), format!(" 용량   - ^{}^", SIZES[self.size].0), "-".into(), format!(" 방식   - ^{}^", mode), format!(" 검사   - ^{}^", onoff(self.verify)), format!(" 방향   - ^{}^", dir)]
        } else {
            vec![format!(" Drive  - ^{}^", self.drive), format!(" Size   - ^{}^", SIZES[self.size].0), "-".into(), format!(" Mode   - ^{}^", mode), format!(" Verify - ^{}^", onoff(self.verify)), format!(" Direct - ^{}^", dir)]
        }
    }

    /// Formats the image file; returns a message or an error.
    fn format(&mut self) -> Result<String, String> {
        let p = Path::new(&self.drive);
        let g = if self.quick {
            let d = std::fs::read(p).map_err(|_| t("디스켓의 정보를 읽지 못합니다", "Reading error...").to_string())?;
            Geo::from_boot(&d).ok_or_else(|| t("디스켓의 정보를 읽지 못합니다", "Disk read error").to_string())?
        } else {
            Geo::from_size(self.size)
        };
        let serial = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as u32).unwrap_or(0x1998_1221);
        let sys = system_area(&g, serial);
        let total = g.total as usize * 512;
        let mut img = if self.quick { std::fs::read(p).map_err(|e| e.to_string())? } else { vec![0xF6u8; total] };
        img.resize(total, 0xF6);
        img[..sys.len()].copy_from_slice(&sys);
        if let Some(d) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(p, &img).map_err(|_| t("쓰기방지탭이 붙어 있습니다", "Write protected error").to_string())?;
        let last = g.tracks().saturating_sub(1);
        self.progress = Some(format!("{} ^{}^ {} ^{}^", t("트랙:", "Track:"), if self.forward { last } else { 0 }, t("/  헤드:", "/  Head:"), g.heads - 1));
        if self.verify {
            let back = std::fs::read(p).map_err(|e| e.to_string())?;
            if back.len() != total || back[..sys.len()] != sys[..] {
                return Err(t("부트섹터를 초기화할 수 없습니다", "Boot Sector initalize error...").into());
            }
        }
        Ok(format!("{} {} bytes {}", t("포맷 완료:", "Format complete:"), commas(g.data_bytes()), t("사용 가능", "available")))
    }

    fn act(&mut self, i: usize) -> Flow {
        match i {
            0 => {
                let lines = [self.drive.clone(), t("이 이미지 파일을 포맷합니다. 계속할까요?", "Format this image file. Continue?").to_string()];
                let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
                self.modal = Some((Modal::yes_no(t("포맷", "Format"), &refs, true), What::Confirm));
            }
            2 => self.modal = Some((Modal::input(t("드라이브 선택", "Drive"), &[t("디스크 이미지 파일 (.IMG)", "Disk image file (.IMG)")], &self.drive), What::Drive)),
            3 => self.size = (self.size + 1) % SIZES.len(),
            4 => self.quick = !self.quick,
            6 => self.verify = !self.verify,
            7 => self.forward = !self.forward,
            8 => return Flow::Quit,
            _ => {}
        }
        Flow::Cont
    }
}

impl Tui for Mf {
    fn draw(&mut self, b: &mut Buffer) {
        let menu = self.menu();
        let info = self.info();
        let title = t("^M^-dir 포맷 (이미지 파일)", "^M^-dir Format (image files)");
        let status = t(" ^Ins^-포맷  ^F3^-드라이브 ^F4^-용량 ^F5^-방식 ^F6^-검사 ^F7^-방향  ^Alt-X^-종료", " ^Ins^-Format  ^F3^-Drive ^F4^-Size ^F5^-Mode ^F6^-Verify ^F7^-Direction  ^Alt-X^-Quit");
        toolui::draw(b, &self.colors, &Tool { title, menu: &menu, sel: self.sel, info: &info, status, progress: self.progress.as_deref() });
        if let Some((m, _)) = self.modal.as_mut() {
            m.draw(b, &self.colors);
        }
    }

    fn key(&mut self, kk: KeyEvent) -> Flow {
        if let Some((mut m, w)) = self.modal.take() {
            match (m.key(kk), w) {
                (MRes::Keep, _) => self.modal = Some((m, w)),
                (MRes::Text(s), What::Drive) if !s.trim().is_empty() => self.drive = s.trim().to_string(),
                (MRes::Choice(0), What::Confirm) => {
                    let r = self.format();
                    let (title, msg) = match r {
                        Ok(s) => (t("포맷", "Format"), s),
                        Err(e) => (t("에러", "Error"), e),
                    };
                    self.modal = Some((Modal::msg(title, &[&msg]), What::Info));
                }
                _ => {}
            }
            return Flow::Cont;
        }
        match kk.code {
            KeyCode::Up => {
                self.sel = (self.sel + MENU_N - 1) % MENU_N;
                if self.sel == 1 || self.sel == 5 {
                    self.sel -= 1;
                }
            }
            KeyCode::Down => {
                self.sel = (self.sel + 1) % MENU_N;
                if self.sel == 1 || self.sel == 5 {
                    self.sel += 1;
                }
            }
            KeyCode::Enter => return self.act(self.sel),
            KeyCode::Insert => return self.act(0),
            KeyCode::F(3) => return self.act(2),
            KeyCode::F(4) => return self.act(3),
            KeyCode::F(5) => return self.act(4),
            KeyCode::F(6) => return self.act(6),
            KeyCode::F(7) => return self.act(7),
            KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('x') | KeyCode::Char('X') if alt(&kk) => return Flow::Quit,
            _ => {}
        }
        Flow::Cont
    }
}

fn self_test(out: &Path) -> bool {
    let img = std::env::temp_dir().join(format!("mf-test-{}.img", std::process::id()));
    let mut m = Mf { drive: img.display().to_string(), size: 3, quick: false, verify: true, forward: true, sel: 0, modal: None, progress: None, colors: Colors::default() };
    let none = KeyModifiers::NONE;
    let mut r = Report::new();
    m.key(k(KeyCode::Down, none));
    m.key(k(KeyCode::Down, none));
    r.step("move", m.sel == 3, format!("Down x2 moved the menu bar to item {} (skipping the separator)", m.sel + 1));
    m.key(k(KeyCode::Insert, none));
    m.key(k(KeyCode::Enter, none));
    let d = std::fs::read(&img).unwrap_or_default();
    let ok = d.len() == 1_474_560 && d[510] == 0x55 && d[511] == 0xAA && &d[54..62] == b"FAT12   " && d[512] == 0xF0;
    m.modal = None;
    m.key(k(KeyCode::F(5), none));
    m.key(k(KeyCode::Insert, none));
    m.key(k(KeyCode::Enter, none));
    let quick_ok = matches!(&m.modal, Some((Modal::Msg { title, .. }, _)) if title != t("에러", "Error"));
    r.step("act", ok && quick_ok, format!("Ins formatted a 1.44M FAT12 image ({} bytes, boot signature, media F0); quick format of it worked: {}", d.len(), quick_ok));
    m.modal = None;
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    m.draw(&mut b);
    let s = screen_text(&b);
    r.step("hud", s.contains("INFO") && s.contains("1.44M"), "menu and INFO box (drive, size, mode, verify, direction) are drawn");

    // mouse: a command click runs it, a status-line hint click presses its key
    use mdir_core::mouse::click;
    mdir_core::text::set_korean(false);
    let (v0, f0) = (m.verify, m.forward);
    let c1 = click(&mut m, "Verify            F6", false);
    let c2 = click(&mut m, "F7-Direction", false);
    let c3 = click(&mut m, "F3-Drive", false);
    let drive_box = matches!(&m.modal, Some((Modal::Input { .. }, _)));
    r.step(
        "mouse",
        c1 && m.verify != v0 && c2 && m.forward != f0 && c3 && drive_box,
        format!("Verify click toggled verify: {}, Direction click toggled direction: {}, F3 hint opened the drive box: {}", m.verify != v0, m.forward != f0, drive_box),
    );
    let _ = std::fs::remove_file(&img);
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
    let drive = cli.rest.iter().find(|a| !a.starts_with('/')).cloned().unwrap_or_else(|| "A.IMG".into());
    let verify = !cli.rest.iter().any(|a| a == "/-");
    let mut m = Mf { drive, size: 3, quick: false, verify, forward: true, sel: 0, modal: None, progress: None, colors: cfg.colors.clone() };
    if let Some(p) = cli.screenshot {
        if let Err(e) = term::screenshot(&mut m, &p) {
            eprintln!("{}", e);
            std::process::exit(1);
        }
        return;
    }
    if let Err(e) = term::run(&mut m) {
        eprintln!("{}", e);
    }
}
