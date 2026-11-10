//! Terminal loop shared by every tool, plus running DOS-style commands outside the full screen.
use crate::text::t;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyboardEnhancementFlags, MouseButton, MouseEventKind, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use ratatui::crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::crossterm::{cursor, execute};
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

pub enum Flow {
    Cont,
    Quit,
    Run(RunReq),
}

#[derive(Clone, Debug, Default)]
pub struct RunReq {
    pub cmd: String,
    pub cwd: PathBuf,
    /// Wait for a key afterwards ("%%" or the pause settings).
    pub pause: bool,
    /// Leave the program and run it (Alt-SPACE).
    pub quit_after: bool,
}

pub trait Tui {
    fn draw(&mut self, b: &mut Buffer);
    fn key(&mut self, k: KeyEvent) -> Flow;
    /// Called about four times a second when no key arrives (screen saver timers, scans).
    fn tick(&mut self) -> Flow {
        Flow::Cont
    }
    /// The part of the terminal the program uses (e.g. a fixed 25-line screen).
    fn area(&self, full: Rect) -> Rect {
        full
    }
    /// After a command ran, so the program can re-read its directory.
    fn after_run(&mut self) {}
    /// Whether to take mouse events (M.CFG MOUSE).
    fn mouse(&self) -> bool {
        true
    }
}

/// Feeds keys to the program until one of them doesn't just continue.
fn feed<T: Tui>(app: &mut T, keys: Vec<KeyEvent>) -> Flow {
    for k in keys {
        match app.key(k) {
            Flow::Cont => {}
            f => return f,
        }
    }
    Flow::Cont
}

/// Lines a wheel notch scrolls.
const WHEEL: usize = 3;
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

pub fn shell(cmd: &str) -> Command {
    if cfg!(windows) {
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
        let mut c = Command::new(comspec);
        c.arg("/C").arg(cmd);
        c
    } else {
        let sh = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let mut c = Command::new(sh);
        c.arg("-c").arg(cmd);
        c
    }
}

fn enter(enh: bool, mouse: bool) -> io::Result<()> {
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
    if mouse {
        execute!(io::stdout(), EnableMouseCapture)?;
    }
    if enh {
        let _ = execute!(io::stdout(), PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES));
    }
    Ok(())
}

fn leave(enh: bool) -> io::Result<()> {
    let _ = execute!(io::stdout(), DisableMouseCapture);
    if enh {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    execute!(io::stdout(), LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()
}

pub fn wait_key() {
    let _ = terminal::enable_raw_mode();
    loop {
        if let Ok(Event::Key(k)) = event::read() {
            if k.kind == KeyEventKind::Press {
                break;
            }
        }
    }
    let _ = terminal::disable_raw_mode();
}

/// Runs a command on the normal screen. Empty command = just show that screen (Ctrl-T).
pub fn run_outside(r: &RunReq) -> Option<i32> {
    let mut code = None;
    if !r.cmd.trim().is_empty() {
        println!();
        let _ = io::stdout().flush();
        code = shell(&r.cmd).current_dir(&r.cwd).status().ok().and_then(|s| s.code());
    }
    if r.pause || r.cmd.trim().is_empty() {
        print!("\n{}", t("  아무 키나 누르세요 - M-dir로 돌아갑니다", "  Press any key to return to M-dir"));
        let _ = io::stdout().flush();
        wait_key();
    }
    code
}

pub fn run<T: Tui>(app: &mut T) -> io::Result<()> {
    let enh = matches!(terminal::supports_keyboard_enhancement(), Ok(true));
    let mouse = app.mouse();
    enter(enh, mouse)?;
    let mut last_click: Option<(u16, u16, Instant)> = None;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |p| {
        let _ = leave(true);
        hook(p);
    }));
    let mut term = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    term.clear()?;
    let res = (|| -> io::Result<()> {
        loop {
            term.draw(|f| {
                let full = f.area();
                let a = app.area(full).intersection(full);
                let mut sub = Buffer::empty(a);
                crate::mouse::clear();
                app.draw(&mut sub);
                f.buffer_mut().merge(&sub);
            })?;
            let flow = if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => app.key(k),
                    Event::Mouse(m) => match m.kind {
                        MouseEventKind::ScrollUp => feed(app, vec![crate::mouse::key(KeyCode::Up); WHEEL]),
                        MouseEventKind::ScrollDown => feed(app, vec![crate::mouse::key(KeyCode::Down); WHEEL]),
                        MouseEventKind::Down(MouseButton::Left) => {
                            let now = Instant::now();
                            let double = matches!(last_click, Some((x, y, t)) if x == m.column && y == m.row && now.duration_since(t) < DOUBLE_CLICK);
                            last_click = if double { None } else { Some((m.column, m.row, now)) };
                            feed(app, crate::mouse::keys_at(m.column, m.row, double))
                        }
                        _ => Flow::Cont,
                    },
                    _ => Flow::Cont,
                }
            } else {
                app.tick()
            };
            match flow {
                Flow::Cont => {}
                Flow::Quit => return Ok(()),
                Flow::Run(r) => {
                    leave(enh)?;
                    if r.quit_after {
                        let code = run_outside(&r);
                        std::process::exit(code.unwrap_or(0));
                    }
                    run_outside(&r);
                    enter(enh, mouse)?;
                    term.clear()?;
                    app.after_run();
                }
            }
        }
    })();
    leave(enh)?;
    res
}

/// Config / data folder: $M_CFG's folder, else ~/.config/mdir (Unix) or %APPDATA%\mdir (Windows).
pub fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("M_CFG") {
        if let Some(d) = PathBuf::from(p).parent() {
            return d.to_path_buf();
        }
    }
    let base = if cfg!(windows) {
        std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir())
    } else {
        std::env::var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|_| {
            std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")).unwrap_or_else(|_| std::env::temp_dir())
        })
    };
    base.join("mdir")
}

/// Folder of the running program (%M), where the other tools live.
pub fn exe_dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_default()
}

/// Parses "--screenshot <file>" and "--self-test <file>" for any tool.
pub struct Cli {
    pub screenshot: Option<PathBuf>,
    pub self_test: Option<PathBuf>,
    pub rest: Vec<String>,
}

pub fn cli() -> Cli {
    let mut c = Cli { screenshot: None, self_test: None, rest: vec![] };
    // output paths are fixed now: the tools change their working directory as they browse
    let abs = |p: String| -> PathBuf { std::path::absolute(&p).unwrap_or_else(|_| PathBuf::from(p)) };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--screenshot" => c.screenshot = it.next().map(abs),
            "--self-test" => c.self_test = it.next().map(abs),
            _ => c.rest.push(a),
        }
    }
    c
}

/// Draws the program about 120 times into an 80x25 screen and saves the picture.
pub fn screenshot<T: Tui>(app: &mut T, path: &std::path::Path) -> Result<(), String> {
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    for _ in 0..120 {
        b = Buffer::empty(Rect::new(0, 0, 80, 25));
        app.draw(&mut b);
    }
    if std::env::var("MDIR_DUMP").is_ok() {
        eprintln!("{}", crate::scr::screen_text(&b));
    }
    crate::shot::save_png(&b, path)
}

/// Self-test report in Craft Anything's format.
pub struct Report {
    pub steps: Vec<(String, bool, String)>,
}

impl Report {
    pub fn new() -> Self {
        Report { steps: vec![] }
    }
    pub fn step(&mut self, name: &str, ok: bool, detail: impl Into<String>) {
        self.steps.push((name.into(), ok, detail.into()));
    }
    pub fn save(&self, path: &std::path::Path) -> io::Result<()> {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ");
        let items: Vec<String> = self
            .steps
            .iter()
            .map(|(n, ok, d)| format!("{{\"step\": \"{}\", \"passed\": {}, \"detail\": \"{}\"}}", esc(n), ok, esc(d)))
            .collect();
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        std::fs::write(path, format!("{{\"version\": 1, \"steps\": [\n  {}\n]}}\n", items.join(",\n  ")))
    }
    pub fn all_passed(&self) -> bool {
        self.steps.iter().all(|s| s.1)
    }
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

/// A key press for scripted tests.
pub fn k(code: event::KeyCode, m: event::KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, m)
}
