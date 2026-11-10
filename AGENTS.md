# AGENTS.md

Guide for contributors and AI coding agents. Read it before your first change; it is meant to be
enough context on its own. (Claude Code loads it through `CLAUDE.md`.)

## What this is

M-dir reimplements **MdirIII 3.10** (1998, a Korean DOS file manager) as terminal programs in Rust
with ratatui + crossterm. The goal is the original's look and keys, on today's Linux, macOS and
Windows terminals. Users are mostly Korean; every message exists in Korean and English.

## Rules that must not be broken

1. **No original material in the repo.** Never add MdirIII's program files, manuals (`*.DOC`), help
   files (`*.HLP`), colour files (`*.COL`), logos, or text copied from them (help pages, usage
   screens, manuals). Write new text in your own words. Short UI labels needed for familiarity
   (menu items, key names) are the existing exception; don't add long copied passages.
2. **No original branding.** Never show "MDIR COMPUTER", the original author's name, copyright lines
   or version "3.10" as if this were the original. "MdirIII" may appear only to describe
   compatibility.
3. **No copied code.** Don't paste decompiled or disassembled code. Describe behaviour (a rule, a
   number, a layout) and write the Rust yourself. Record where a value was confirmed in
   `MECHANICS.md` (see below).
4. **No secrets or personal paths** in code, tests, docs or commits (no `/home/<name>/…`).
5. **Licence:** contributions are `MIT OR Apache-2.0`.

## Layout

```text
Cargo.toml            workspace; package `mdir` builds the `m` binary from src/
src/                  m — main file manager
  main.rs             command line (/options), usage text, starts App
  app.rs              App state and the command behind every key (Act); modal results
  act.rs              key → Act table and the six pull-down menus
  draw.rs             main screen: function-key line, Path line, file list, count line, status line,
                      pull-down menu, help, attribute box
  list.rs             directory → entries, filters, sorting, one entry's text per column mode
  mcd.rs              MCD directory tree (F10)
  ops.rs              copy/move/delete with the overwrite prompts
  arc.rs              archives: ZIP built in, others through external archivers
  help.rs             built-in F1 help (KO/EN)
  selftest.rs         --self-test scenario for m
crates/core/          mdir-core, shared by every tool
  term.rs             terminal loop, `Tui` trait, Flow, running shell commands, --screenshot/--self-test CLI
  mouse.rs            click zones (see "Mouse")
  modal.rs            dialogs: Msg, Ask, Input (LineEdit), Menu
  scr.rs              DOS-style drawing: put(x, y, text, attr), put_hl with ^hot^ keys, frames, shadows
  pal.rs              DOS attributes (fg | bg<<4), VGA palette, built-in colour sets, .COL read/write
  cfg.rs              M.CFG (text format): options table, function keys, extension programs, QCD
  text.rs             t(ko, en), KS/Johab/CP437 decoding, display width, number/date formatting
  fsx.rs              attributes, drives, DESCRIPT.ION
  shot.rs             renders a buffer to PNG (used by --screenshot)
  toolui.rs           shared screen of mf and mcopy
crates/vv, mff, mset, mcopy, mf, dscman   one binary each (see README table)
MECHANICS.md / mechanics.json   every rule and number, with its source or "estimated"
tools/gen_mechanics.py          generates both files from one table
shots/                          screenshots used by README
```

## How the code works

- **One loop for all tools.** Each program implements `term::Tui` (`draw`, `key`, `tick`, `area`,
  `after_run`, `mouse`) and calls `term::run`. `key` returns `Flow::Cont`, `Flow::Quit` or
  `Flow::Run(RunReq)` (leave full screen, run a shell command, come back).
- **Drawing is DOS-like.** Draw with `scr::put(b, x, y, text, attr)` where `attr` is a DOS attribute
  byte; colours come from `Colors.items[pal::…]`, never hard-coded RGB. `^text^` in `put_hl`
  marks highlighted hot keys.
- **Text:** use `t("한글", "English")` for every user-visible string. Korean characters are two
  cells wide: use `text::width`/`fit`/`hw`, never `.len()`, for layout.
- **Dialogs** are `modal::Modal` values kept in the app's `modal` field; `Modal::key` returns
  `MRes` (Keep, Close, Choice, Text, Unhandled) and the app acts on it.
- **Settings** live in `M.CFG` (`$M_CFG`, else `~/.config/mdir/` or `%APPDATA%\mdir\`). Options are
  declared once in the `cfg.rs` table (`o!` lines) and edited by mset.

### Mouse

Clicks never have their own logic. While drawing, code registers zones with
`mouse::zone(x, y, w, h, keys, double_keys)`; a left click replays the keys of the topmost zone
under the pointer (double click adds `double_keys`). Dialogs and menus first call `mouse::block`
over the whole screen so clicks can't reach what's underneath. The wheel sends Up/Down ×3. To make
something clickable, register a zone next to the code that draws it, with the keys a keyboard user
would press; for list rows use `mouse::select(k)` (Home + Down×k) or `mouse::choose(k)` (… + Enter).
M's mouse can be turned off with the M.CFG option `MOUSE`.

Clickable today: m's file list (click = move bar, double click = Enter), function-key line,
F10/F11/F12 on the status line, pull-down menus, F1 help (click closes), the MCD tree (click =
move bar via `Mcd::keys_to`, double click = go there) and its function-key line, and every `Modal`
in every tool; vv's status line (code field = F9, F1 = help, /U turns the mouse off); mset's menu bar,
drop-downs, list editors (click = select, double click = Enter) and colour picker (click = pick,
double click = choose); mff's file list (click = move bar, double click = choose); mf and mcopy's
command menu (click = run, via `toolui`) and the `^Key^-label` hints on their status line.

## Build and verify

```sh
cargo build --release
cargo test --workspace
for b in m vv mff mset mcopy mf dscman; do ./target/release/$b --self-test /tmp/$b.json || echo "$b FAILED"; done
./target/release/m --screenshot /tmp/m.png        # look at the picture after layout changes
MDIR_LANG=en ./target/release/m --screenshot /tmp/m-en.png
```

- Every tool supports `--self-test <json>` (scripted keys in a temp folder, three or more steps)
  and `--screenshot <png>`. When you add behaviour, add a step to that tool's self-test; for m it
  is `src/selftest.rs`. `mouse::click(&mut app, "text", double)` clicks what the screen shows.
- Check both languages and an 80×25 screen when you change layout.
- `cargo clippy --workspace --all-targets` is clean; keep it that way.
- The code is **not** rustfmt-formatted. Don't reformat files you aren't otherwise changing; keep
  diffs to what your change needs.

## MECHANICS.md

Rules and numbers (screen rows, column widths, colours, prompts, limits) are listed with their
source: a manual section, or "estimated" when nothing settled it. To change one, edit the table in
`tools/gen_mechanics.py`, run `python3 tools/gen_mechanics.py` from the repo root, and commit the
regenerated `MECHANICS.md` and `mechanics.json` with the code. If you confirm an estimated value
(for example from a DOSBox screenshot of the original), replace "estimated" with how you confirmed it.

## Known gaps (good first issues)

- Values still estimated: tagged-file colour, Esc's default action, delete prompt's default,
  screen saver delay (see MECHANICS.md).
- The Alt/Ctrl hint line can't appear: terminals don't report a modifier pressed alone.
- Old binary `M.CFG` files can't be read.
- Archives other than ZIP need the archiver installed.

## Pull requests

- Branch from `main` (`feature/…`, `fix/…`, `docs/…`); one topic per PR.
- In the PR, say what you verified (tests, self-tests, screenshots) and what you couldn't.
- Update README (both languages) when user-visible behaviour changes, and MECHANICS.md when a rule
  or number changes.

## Releases

Bump `version` in `Cargo.toml`, merge, then push a tag `vX.Y.Z` on `main`.
`.github/workflows/release.yml` creates the GitHub Release, runs the tests (and the self-tests on
Linux) and uploads archives for Linux x86_64, Windows x86_64 and macOS arm64/x86_64.
