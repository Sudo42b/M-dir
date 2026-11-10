//! Mouse support. Drawing code registers click zones; a click replays the keys of the topmost zone
//! under the pointer, so every mouse action goes through the same code as the keyboard.
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::cell::RefCell;

struct Zone {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    keys: Vec<KeyEvent>,
    double: Vec<KeyEvent>,
}

thread_local! {
    static ZONES: RefCell<Vec<Zone>> = const { RefCell::new(Vec::new()) };
}

/// Forget the zones of the last frame (called before each draw).
pub fn clear() {
    ZONES.with(|z| z.borrow_mut().clear());
}

/// Register a zone. `keys` run on a click; `double` runs after them on a double click.
/// Zones registered later lie on top.
pub fn zone(x: u16, y: u16, w: u16, h: u16, keys: Vec<KeyEvent>, double: Vec<KeyEvent>) {
    ZONES.with(|z| z.borrow_mut().push(Zone { x, y, w, h, keys, double }));
}

/// A zone that swallows clicks, so a dialog keeps clicks away from what's under it.
pub fn block(x: u16, y: u16, w: u16, h: u16, keys: Vec<KeyEvent>) {
    zone(x, y, w, h, keys, vec![]);
}

/// The keys for a click at (x, y): the topmost zone's keys, plus its double-click keys if `double`.
pub fn keys_at(x: u16, y: u16, double: bool) -> Vec<KeyEvent> {
    ZONES.with(|z| {
        z.borrow()
            .iter()
            .rev()
            .find(|z| x >= z.x && x < z.x.saturating_add(z.w) && y >= z.y && y < z.y.saturating_add(z.h))
            .map(|z| {
                let mut k = z.keys.clone();
                if double {
                    k.extend(z.double.iter().cloned());
                }
                k
            })
            .unwrap_or_default()
    })
}

pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// `n` presses of one key.
pub fn times(code: KeyCode, n: usize) -> Vec<KeyEvent> {
    vec![key(code); n]
}

/// Keys that put a list's bar on item `k`: Home, then Down k times.
pub fn select(k: usize) -> Vec<KeyEvent> {
    let mut ks = vec![key(KeyCode::Home)];
    ks.extend(times(KeyCode::Down, k));
    ks
}

/// `select(k)` followed by Enter.
pub fn choose(k: usize) -> Vec<KeyEvent> {
    let mut ks = select(k);
    ks.push(key(KeyCode::Enter));
    ks
}

/// Self-test helper: draws `app` on an 80×25 screen, finds `text` (bottom row first) and clicks
/// it through the registered zones. False when the text isn't on the screen.
pub fn click<T: crate::term::Tui>(app: &mut T, text: &str, double: bool) -> bool {
    clear();
    let mut b = ratatui::buffer::Buffer::empty(ratatui::layout::Rect::new(0, 0, 80, 25));
    app.draw(&mut b);
    let Some((x, y)) = find_text(&b, text) else { return false };
    for k in keys_at(x, y, double) {
        app.key(k);
    }
    true
}

/// Keys that move a bar from `from` to `to` with Up/Down.
pub fn move_bar(from: usize, to: usize) -> Vec<KeyEvent> {
    if to >= from {
        times(KeyCode::Down, to - from)
    } else {
        times(KeyCode::Up, from - to)
    }
}

/// Where `text` starts on a drawn buffer, searching from the bottom row up (so a list or tree wins
/// over a path line above it). Used by self-tests to click on what the screen shows.
pub fn find_text(b: &ratatui::buffer::Buffer, text: &str) -> Option<(u16, u16)> {
    let a = b.area;
    let want: Vec<String> = text.chars().map(|c| c.to_string()).collect();
    let n = want.len() as u16;
    (a.top()..a.bottom()).rev().find_map(|y| {
        (a.left()..a.right().saturating_sub(n.saturating_sub(1)))
            .find(|&x| want.iter().enumerate().all(|(i, w)| b[(x + i as u16, y)].symbol() == w))
            .map(|x| (x, y))
    })
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn topmost_zone_wins() {
        clear();
        zone(0, 0, 10, 10, vec![key(KeyCode::Down)], vec![key(KeyCode::Enter)]);
        block(2, 2, 3, 3, vec![key(KeyCode::Esc)]);
        assert_eq!(keys_at(0, 0, false), vec![key(KeyCode::Down)]);
        assert_eq!(keys_at(0, 0, true), vec![key(KeyCode::Down), key(KeyCode::Enter)]);
        assert_eq!(keys_at(3, 3, true), vec![key(KeyCode::Esc)]);
        assert!(keys_at(20, 20, false).is_empty());
        assert_eq!(move_bar(5, 2), times(KeyCode::Up, 3));
        assert_eq!(choose(2), vec![key(KeyCode::Home), key(KeyCode::Down), key(KeyCode::Down), key(KeyCode::Enter)]);
    }
}
