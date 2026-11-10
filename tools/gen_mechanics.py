"""Writes MECHANICS.md and mechanics.json from one table, so both stay the same.
Run from app/: python3 tools/gen_mechanics.py"""
import json

V = [
    # group, name, value, unit, source (None = estimated)
    ("screen", "screen size (fixed mode)", "80x25", "cells", "M.HLP Ctrl-E 25/28/43/50"),
    ("screen", "rows: function keys / Path / line / list / line / count / status", "1 / 1 / 1 / 19 / 1 / 1 / 1", "rows", "M.EXE 0xb6e4 (list = lines-6, or lines-4 without the two lines), 0x33c8 (Path on row 1), 0x300a (list from row 3)"),
    ("screen", "file list rows at 25 lines", 19, "rows", "M.EXE 0xb6e4: e3d = e4f - 5 (+2 when Alt-\\ hides the lines)"),
    ("screen", "Path line", "label+path in count colour, cut to 52 cells; rest of row in count highlight; Volume [..] / Work .. right-aligned", "", "M.EXE seg 114b0:2730"),
    ("screen", "count line", "files Str(:5) File, dirs Str(:5) Dir, bytes (14, commas) Byte, free (14)(pct%)byte free (total K/M)", "", "M.EXE seg 114b0:29bf, number format seg 25230:0512"),
    ("screen", "free percentage", "free div (total div 100)", "", "M.EXE seg 114b0:2b34"),
    ("screen", "disk size unit", "K up to 0x200B20 bytes, M above", "", "M.EXE seg 114b0:2b8x"),
    ("screen", "sort letter", "NESTBQ at column 79, colour = count bg + index + 9 (ascending) or + index (descending)", "", "M.EXE seg 114b0:2c3f"),
    ("screen", "hidden-files mark", "H at column 80, bright green on count-highlight bg", "", "M.EXE seg 114b0:2cb5"),
    ("screen", "status line", "size(9)│YY-MM-DD│HH:MM│attr(4)│clock or n File bytes Byte│F10=MCD│F11=QCD│F12=메뉴", "", "M.EXE 0x26f0, strings 0x24e5-0x2561"),
    ("screen", "status clock", "YY-MM-DD, day (일..토 / SUN..SAT), H:MM:SS (+am/pm when 12-hour), refreshed about every 0.5 s (BIOS ticks div 9)", "", "M.EXE 0x212e"),
    ("screen", "Alt/Ctrl hint line", "BIOS shift flags 0040:0017 bits 2-3 replace the status line with the Alt or Ctrl hint list", "", "M.EXE 0x26f0"),
    ("screen", "selection bar", "black text on the item's colour ((fg&15)<<4), 0x70 when that is 0 or in mono", "attr", "M.EXE 0x33c8"),
    ("screen", "size column", "11 cells with commas, {MB}M from 1,000,000,000; mode 4: 7 cells, plain below 1,000,000 else tenths of MB + M", "", "M.EXE seg 114b0:0633"),
    ("screen", "size column colour", "other-files colour; read-only/system yellow (+14), hidden bright magenta (+13); unit M bright red (+12)", "attr", "M.EXE seg 114b0:0633"),
    ("screen", "directory size text", "' [ SubDir ]', ' [ Up-Dir ]', ' [ CurDir ]', mode 4 '   DIR '", "", "M.EXE seg 114b0:0604-0624"),
    ("screen", "scroll bar", "last column over the list; track 0xB0, thumb 0x88 (mono 0x70/0x07), thumb = rows*page/total", "", "M.EXE 0x300a"),
    ("screen", "entry width in 2-column mode", 38, "cells", "M.DOC sample 'COMMAND  COM     52,925 93-03-10  6:00'"),
    ("screen", "columns per line", "1-6, auto", "", "M.HLP Alt-1..6, Alt-0"),
    ("screen", "auto columns max without 5,6 mode", 4, "columns", "MSET.DOC 컬럼수 자동조정 (5,6모드 질문)"),
    ("screen", "list order", "column-major (down, then right)", "", "M.DOC sample screen"),
    ("screen", "order of entries", "Up-Dir, dirs, files, drives; dirs and files sorted separately", "", "M.EXE seg 114b0:3e2e"),
    ("sort", "comparators", "name: first char, 8-char name, then ext; exec: BAT, BTM, COM, EXE first, then by name", "", "M.EXE seg 114b0:2e1f, 319b (strings 318b)"),
    ("sort", "ascending / descending", "separate comparator per sort (asc 2e1f..319b, desc 357d..3933)", "", "M.EXE seg 114b0:3e2e"),
    ("colors", "colour item → variable", "MCD e41, MCD status e42, dir e3e, drive e40, lines e3f, fkey e46/e47, count e4b/e4a, files e48, help e43, search e44/e50, dialog e45", "", "M.EXE seg 0cc50:1688-1777 (config load)"),
    ("colors", "default colour file", "BLACK.COL", "", "MSET.DOC 색깔부르기 (BLACK.COL 초기치); BLACK.COL offset 0x48"),
    ("colors", "directory", "0C", "attr", "BLACK.COL 0x4C"),
    ("colors", "drive name", "06", "attr", "BLACK.COL 0x4E"),
    ("colors", "main screen lines", "03", "attr", "BLACK.COL 0x50"),
    ("colors", "function key normal / highlight", "3F / 3E", "attr", "BLACK.COL 0x52, 0x54"),
    ("colors", "count line normal / highlight", "70 / 7F", "attr", "BLACK.COL 0x56, 0x58"),
    ("colors", "other files", "07", "attr", "BLACK.COL 0x5A"),
    ("colors", "help", "6E", "attr", "BLACK.COL 0x5C"),
    ("colors", "speed search main / MCD", "1F / 5F", "attr", "BLACK.COL 0x5E, 0x60"),
    ("colors", "dialog", "9F", "attr", "BLACK.COL 0x62"),
    ("colors", "MCD tree / status", "9F / F4", "attr", "BLACK.COL 0x48, 0x4A"),
    ("colors", "EXE / COM / BAT", "0A / 0B / 0E", "attr", "BLACK.COL extension table 0x85.. (names at 0x11B)"),
    ("colors", "archives", "0D", "attr", "BLACK.COL extension table"),
    ("colors", "tagged file", "blue background", "", None),
    ("limits", "files per directory", "no limit (original 2000)", "", "M.DOC 제한사항: 2000; not needed today"),
    ("limits", "MCD directories per drive", 20000, "dirs", "original 2000 (M.DOC 제한사항); raised for modern disks"),
    ("limits", "description length", 80, "chars", "M.DOC 주석: 입력/수정시 최대 80자"),
    ("limits", "dos command history", 20, "commands", "M.DOC 도스명령: 최대 20개"),
    ("limits", "QCD directories", 10, "dirs", "M.DOC QCD"),
    ("limits", "function keys", 28, "keys", "M.DOC / MSET.DOC: F2-F9, Ctrl-F1..F10, Alt-F1..F10"),
    ("limits", "commands per key/extension", 20, "commands", "MSET.DOC: ;로 구분해 최대 20개, 255자"),
    ("limits", "extension programs", 100, "extensions", "MSET.DOC 확장자지정 최대 100개"),
    ("limits", "filter specs", 15, "specs", "M.DOC Alt-F: 최대 15개"),
    ("limits", "%[EXT] nesting before 'Bad program definition'", 8, "levels", None),
    ("keys", "speed search", "lower-case letters, digits; Ctrl-L next; BS edit; Esc cancel", "", "M.DOC 선택막대"),
    ("keys", "drive change", "Shift-letter (upper case)", "", "M.DOC 드라이브"),
    ("keys", "PgUp/PgDn", "first/last on screen, then previous/next screen", "", "M.DOC 선택막대"),
    ("keys", "Esc on main screen", "nothing (MSET choice)", "", None),
    ("keys", "delete prompt default", "No", "", None),
    ("keys", "auto line (Alt-=)", "use all terminal lines (off = 80x25)", "", "M.HLP Alt-= fit lines on screen automatically"),
    ("keys", "horizontal line (Alt-\\)", "the two lines around the file list", "", "M.DOC Alt-\\ 가로구분선"),
    ("timers", "screen saver delay unit", "SAVETIME minutes x 1092 BIOS ticks (about 60 s)", "", "M.EXE seg 0cc50:15ae-15cf"),
    ("timers", "screen saver delay default", 5, "minutes", None),
    ("timers", "idle poll", 250, "ms", None),
    ("archives", "viewed directly", "ZIP, ZIP SFX", "", "M.DOC 압축파일 (ZIP family read natively here)"),
    ("archives", "other kinds", "LZH ICE ARJ ARC OMP ZOO SQZ RAR UC2 HPK ACE via their programs", "", "M.EXE command table (LHA x /a, ARJ x -a, ...)"),
    ("archives", "default new archive", "ZIP", "", None),
    ("files", "descriptions", "DESCRIPT.ION (4DOS)", "", "M.DOC 주석"),
    ("files", "history file", "M.HIS", "", "M.DOC 파일목록"),
    ("files", "tree file", "MDIRTREE*.DIR in the settings folder", "", "M.DOC 파일목록 (MDIRTREE.DIR)"),
    ("disk tools", "MCOPY/MF target", "disk image files (.IMG)", "", None),
    ("disk tools", "MF sizes", "360K, 720K, 1.2M, 1.44M", "", "MF.DOC 포맷용량"),
    ("disk tools", "MCOPY copies", "1-9999", "copies", "MCOPY.DOC 9999개까지"),
]

NOTES = """
## How the main screen works

- Rows (25-line screen, from the decompiled code): 0 function keys, 1 Path line, 2 line, 3-21 file
  list (19 rows), 22 line, 23 count line, 24 status line. Alt-\\ removes both lines and the list
  grows to 21 rows (M.EXE 0xb6e4).
- Decompiled from an unpacked copy of M.EXE (the original is packed; its image was taken from
  memory after the unpacking stub ran and wrapped as an MZ with rebuilt relocations). Addresses
  "M.EXE 0x...." are offsets in that image; "seg XXXXX:YYYY" are offsets inside the code segment
  that starts at image offset XXXXX.
- Sorting (MSET.DOC table): name→(name,ext); ext→(ext,name); size/time→(key,name,ext);
  colour/exec→(key,ext,name). Descending reverses the first key (first two for colour/exec).
- Copy/move conflicts: OverWrite/All/Skip_all/Skip/Rename/Newer, with source and target size,
  date and time; target highlighted when they differ (M.DOC 복사). Only finished files lose
  their tag. Ctrl-BS then goes to the target directory.
- Delete: asks once; read-only/system/hidden files and non-empty directories ask again
  (Yes/All/No); root directories need MCD with "루트 삭제가능" and three confirmations.
- Program variables (MSET.DOC): !.! %F !, ,! !\\ !: !\\!.! %A %M %1..%0 %[EXT] %; %L; options %B
  %N (ignored today) %P (edit first) %$ (run in program dir) %% (wait for a key). Several programs
  in one definition (;) give a choice box.
- MCD: tree drawn sideways, 16 cells per level; ↑↓ siblings, ←→ parent/child, Ctrl-←→ any
  directory, F2 rescan, F3 drive, F4 move, F5 copy, F6 rename, F7/Ins make, F8/Del delete, F9
  size, F11 QCD, Shift-F1..F10 twice to choose, Space visits QCD dirs, Alt-N sort.
"""

vals = []
for g, n, v, u, src in V:
    d = {"name": n, "group": g, "value": v}
    if u:
        d["unit"] = u
    if src:
        d["source"] = src
    else:
        d["estimated"] = True
    vals.append(d)
with open("mechanics.json", "w", encoding="utf-8") as f:
    json.dump({"version": 1, "values": vals}, f, ensure_ascii=False, indent=1)

md = ["# M-dir — rules and numbers (MdirIII 3.10 behaviour notes)", "",
      'Every value the rebuild uses, with where it came from. "estimated" means the manuals and the program text did not settle it.']
cur = None
for g, n, v, u, src in V:
    if g != cur:
        md += ["", f"## {g}", "", "| value | | unit | source |", "|---|---|---|---|"]
        cur = g
    md.append(f"| {n} | {v} | {u} | {src or 'estimated'} |")
with open("MECHANICS.md", "w", encoding="utf-8") as f:
    f.write("\n".join(md) + "\n" + NOTES)
print(len(vals), "values")
