//! --self-test: drives the main screen with scripted keys in a scratch directory and checks the
//! results from the program's own state.
use crate::app::{App, Mode};
use mdir_core::cfg::Config;
use mdir_core::ratatui::buffer::Buffer;
use mdir_core::ratatui::crossterm::event::{KeyCode, KeyModifiers};
use mdir_core::ratatui::layout::Rect;
use mdir_core::scr::screen_text;
use mdir_core::term::{k, Report, Tui};
use std::path::Path;

/// Draws the screen (registering its click zones) and returns where `text` starts on it, searching
/// from the bottom up so the path line (row 1) loses to the list or tree below it.
fn locate(app: &mut App, text: &str) -> Option<(u16, u16)> {
    mdir_core::mouse::clear();
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    app.draw(&mut b);
    let want: Vec<String> = text.chars().map(|c| c.to_string()).collect();
    (0..25u16).rev().find_map(|y| {
        (0..80u16).find(|&x| want.iter().enumerate().all(|(i, w)| x + (i as u16) < 80 && b[(x + i as u16, y)].symbol() == w)).map(|x| (x, y))
    })
}

/// Left click (or double click) at `text`'s position, through the same zones the terminal uses.
fn click(app: &mut App, text: &str, double: bool) -> bool {
    let Some((x, y)) = locate(app, text) else { return false };
    for key in mdir_core::mouse::keys_at(x, y, double) {
        app.key(key);
    }
    true
}

fn screen(app: &mut App) -> String {
    let mut b = Buffer::empty(Rect::new(0, 0, 80, 25));
    app.draw(&mut b);
    screen_text(&b)
}

pub fn run(out: &Path) -> bool {
    let root = std::env::temp_dir().join(format!("mdir-selftest-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::create_dir_all(root.join("SUB"));
    for (n, s) in [("ALPHA.TXT", 120), ("BETA.DOC", 4000), ("GAMMA.EXE", 52925), ("DELTA.BAT", 33)] {
        let _ = std::fs::write(root.join(n), vec![b'x'; s]);
    }
    // keep the tree file and history away from the person's own settings
    std::env::set_var("M_CFG", root.join("M.CFG"));
    let cfg = Config::defaults(root.join("M.CFG"));
    let mut app = App::new(cfg, root.clone());
    let none = KeyModifiers::NONE;
    let mut r = Report::new();

    // move: the selection bar follows the arrow keys and speed search
    let before = app.cur;
    app.key(k(KeyCode::Down, none));
    app.key(k(KeyCode::Down, none));
    let after = app.cur;
    app.key(k(KeyCode::Char('g'), none));
    let found = app.cur_entry().map(|e| e.name.clone()).unwrap_or_default();
    app.key(k(KeyCode::Esc, none));
    r.step("move", after == before + 2 && found == "GAMMA.EXE", format!("bar moved {} -> {} with Down x2; speed search 'g' found {}", before, after, found));

    // act: tag a file, copy it (Alt-C) into SUB, then enter SUB through MCD (F10)
    app.find("alpha", 0);
    app.key(k(KeyCode::Char(' '), none));
    let tagged = app.entries.iter().filter(|e| e.tag > 0).count();
    app.key(k(KeyCode::Char('c'), KeyModifiers::ALT));
    if let Some((mdir_core::modal::Modal::Input { edit, .. }, _)) = app.modal.as_mut() {
        edit.set(&root.join("SUB").to_string_lossy());
        edit.fresh = false;
    }
    app.key(k(KeyCode::Enter, none));
    let copied = root.join("SUB").join("ALPHA.TXT").is_file();
    app.key(k(KeyCode::F(10), none));
    let in_mcd = matches!(app.mode, Mode::Mcd(_));
    if let Mode::Mcd(m) = &mut app.mode {
        m.goto_path(&root.join("SUB"));
    }
    app.key(k(KeyCode::Enter, none));
    let now = app.dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    r.step(
        "act",
        tagged == 1 && copied && in_mcd && now == "SUB",
        format!("tagged {} file, Alt-C copied ALPHA.TXT into SUB: {}, F10 opened MCD: {}, Enter changed to {}", tagged, copied, in_mcd, now),
    );

    // hud: the screen shows the function-key box, Path line, count line and status box
    app.key(k(KeyCode::Char('.'), none));
    let s = screen(&mut app);
    let checks = [
        ("function keys", s.lines().next().map(|l| l.contains("F2")).unwrap_or(false)),
        ("path", s.contains("경로") || s.contains("Path")),
        ("count line", s.contains(" File") && s.contains(" Dir") && s.contains(" Byte")),
        ("status box", s.contains("F10=MCD") && s.contains("F11=QCD")),
        ("files", s.contains("GAMMA") && s.contains("[ SubDir ]")),
    ];
    let missing: Vec<&str> = checks.iter().filter(|c| !c.1).map(|c| c.0).collect();
    r.step("hud", missing.is_empty(), if missing.is_empty() { "function-key line, Path line, file list, count line and status line are drawn".to_string() } else { format!("missing: {}", missing.join(", ")) });

    // mouse: a click moves the bar, a double click enters a directory, F12 on the status line
    // opens the menu and a click outside it closes it
    let hit = click(&mut app, "BETA", false);
    let on = app.cur_entry().map(|e| e.name.clone()).unwrap_or_default();
    let hit2 = click(&mut app, "SUB", false) && click(&mut app, "SUB", true);
    let entered = app.dir.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let hit3 = click(&mut app, "F12", false);
    let menu = matches!(app.mode, Mode::Menu { .. });
    let hit4 = click(&mut app, "1 File", false);
    let closed = matches!(app.mode, Mode::Main);
    r.step(
        "mouse",
        hit && on == "BETA.DOC" && hit2 && entered == "SUB" && hit3 && menu && hit4 && closed,
        format!("click put the bar on {}, double click entered {}, F12 click opened the menu: {}, click outside closed it: {}", on, entered, menu, closed),
    );

    // mcd mouse: in MCD (opened from SUB) a click on the parent directory moves the bar there and
    // a double click goes to it
    app.key(k(KeyCode::F(10), none));
    let parent_label: String = root.file_name().map(|s| s.to_string_lossy().chars().take(9).collect()).unwrap_or_default();
    let hit5 = click(&mut app, &parent_label, false);
    let bar = if let Mode::Mcd(m) = &app.mode { m.path_of(m.cur) } else { std::path::PathBuf::new() };
    let hit6 = click(&mut app, &parent_label, true);
    let went = app.dir.clone();
    r.step(
        "mcd-mouse",
        hit5 && bar == root && hit6 && went == root,
        format!("click in the tree put the bar on {}, double click went to {}", bar.display(), went.display()),
    );

    let _ = std::fs::remove_dir_all(&root);
    let _ = r.save(out);
    r.all_passed()
}
