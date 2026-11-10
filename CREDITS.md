# Credits and sources

M-dir is an independent reimplementation of the behaviour of **MdirIII 3.10** (1998), the Korean DOS
file manager. It is not affiliated with, endorsed by, or connected to MdirIII or its author.
"MdirIII" is used only to describe compatibility.

## What this repository does not contain

- None of the original program files, manuals, help files or colour files.
- No code from the original programs.

## How the behaviour was worked out

- The original manuals (M.DOC, MSET.DOC, VV.DOC and others) were read for key bindings, screen
  layout and options.
- The original executables were studied to confirm behaviour, key bindings, screen layout and
  number formats. `MECHANICS.md` lists each value and where it was confirmed.
- The F1 help, the VV start screen, usage screens and colour sets are written for this project.
  The two colour sets reproduce the default attribute values so the screen looks familiar.

## Using your own copy

If you own MdirIII, you can put your `M.HLP` / `M-K.HLP` in the M-dir data folder
(`~/.config/mdir/`, or `%APPDATA%\mdir\` on Windows) and F1 shows them instead of the built-in help.
Original `.COL` colour files can be loaded from MSET → 색깔부르기 → 파일에서.

## Built with

- Initial build generated with [Craft Anything](https://github.com/storytold/craft-anything) and
  Claude Code.
- [ratatui](https://ratatui.rs), crossterm, encoding_rs, zip, fontdue, png, chrono.
