//! M.CFG: options, function keys, extension programs, QCD directories and colours, stored as
//! text sections. Option key names follow the ones MSET.EXE knows (VIEW=, FIND=, SAVETIME=, ...).
use crate::pal::{Colors, ITEM_NAMES};
use crate::text::t;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub enum Kind {
    Bool,
    Str,
    Num(i64, i64),
    Choice(&'static [(&'static str, &'static str)]),
}

pub struct Opt {
    pub key: &'static str,
    /// MSET menu: 2 프로그램, 3 메인화면, 4 MCD, 5 기타.
    pub menu: u8,
    pub kind: Kind,
    pub default: &'static str,
    pub ko: &'static str,
    pub en: &'static str,
    pub help_ko: &'static str,
    pub help_en: &'static str,
}

pub const SORTS: &[(&str, &str)] = &[("정렬안함", "None"), ("파일명", "Name"), ("확장자", "Ext"), ("크기", "Size"), ("날짜", "Time"), ("색깔", "Color"), ("실행파일", "Exec")];
pub const ESC_FN: &[(&str, &str)] = &[
    ("작동안함", "Nothing"), ("도스화면보기", "View dos screen"), ("M3종료하기", "Quit M3"), ("도스명령(배치방식)", "Dos command"),
    ("도스명령(스와핑방식)", "Dos command(swap)"), ("메뉴", "Menu"), ("MCD", "MCD"), ("QCD", "QCD"),
    ("상위디렉토리로", "To parent"), ("루트디렉토리로", "To root"),
];
pub const COLUMNS: &[(&str, &str)] = &[("자동조정", "Auto"), ("1", "1"), ("2", "2"), ("3", "3"), ("4", "4"), ("5", "5"), ("6", "6")];
pub const DESC_MODES: &[(&str, &str)] = &[("켜짐", "On"), ("자동", "Auto"), ("꺼짐", "Off")];
pub const HANGUL: &[(&str, &str)] = &[("자동", "Auto"), ("완성형", "KS"), ("조합형", "Johab"), ("영문", "English")];
pub const LINES: &[(&str, &str)] = &[("기본값", "Default"), ("25", "25"), ("28", "28"), ("43/50", "43/50")];
pub const HISDIR: &[(&str, &str)] = &[("임시디렉토리", "Temp dir"), ("M-dir 디렉토리", "M-dir dir")];
pub const ARCHIVES: &[(&str, &str)] = &[("ZIP", "ZIP"), ("ARJ", "ARJ"), ("LZH", "LZH"), ("RAR", "RAR"), ("SQZ", "SQZ"), ("UC2", "UC2"), ("OMP", "OMP"), ("HPK", "HPK"), ("ZOO", "ZOO"), ("ARC", "ARC"), ("ACE", "ACE")];

macro_rules! o {
    ($k:expr, $m:expr, $kind:expr, $d:expr, $ko:expr, $en:expr, $hk:expr, $he:expr) => {
        Opt { key: $k, menu: $m, kind: $kind, default: $d, ko: $ko, en: $en, help_ko: $hk, help_en: $he }
    };
}

pub const OPTIONS: &[Opt] = &[
    o!("VIEW", 2, Kind::Str, "%Mvv !.!", "파일보기", "File viewer", "파일보기 프로그램 (@V로 작동)", "Viewer program (Alt-V)"),
    o!("FORMAT", 2, Kind::Str, "%Mmf", "디스크포맷", "Disk format", "디스크 포맷 프로그램 (^F로 작동)", "Format program (Ctrl-F)"),
    o!("DISKCOPY", 2, Kind::Str, "%Mmcopy", "디스크복사", "Disk copy", "디스크 복사 프로그램 (^C로 작동)", "Disk copy program (Ctrl-C)"),
    o!("PARK", 2, Kind::Str, "", "디스크파킹", "Disk parking", "파킹 프로그램 (없으면 내장 파킹, ^P)", "Parking program (built-in if empty, Ctrl-P)"),
    o!("FIND", 2, Kind::Str, "%Mmff *.*%P", "파일찾기", "File find", "파일찾기 프로그램 (@I로 작동)", "File find program (Alt-I)"),
    o!("SAVER", 2, Kind::Str, "", "화면보호", "Screen saver", "화면보호 프로그램 (완전한 경로, 변수 사용불가)", "Screen saver program (full path, no variables)"),
    o!("COMSPEC", 2, Kind::Str, "", "명령처리기", "Command shell", "명령처리기 (비워두면 자동인식)", "Command processor (auto when empty)"),
    o!("TEMP", 2, Kind::Str, "", "임시디렉토리", "Temp directory", "임시파일 디렉토리 (M.CFG → TEMP → Mdir 디렉토리)", "Temp directory (M.CFG, then TEMP, then Mdir dir)"),
    o!("PAUSE-DOSCMD", 2, Kind::Bool, "ON", "도스명령 정지", "Pause dos command", "도스명령 실행 후 정지", "Wait after a dos command"),
    o!("PAUSE", 2, Kind::Bool, "OFF", "프로그램 정지", "Pause program", "프로그램 실행 후 정지", "Wait after running a program"),
    o!("COLUMN", 3, Kind::Choice(COLUMNS), "0", "컬럼수", "Columns", "한줄에 보여줄 파일 수", "Files per line"),
    o!("COLUMN56", 3, Kind::Bool, "OFF", "5,6 모드 사용", "Use mode 5,6", "자동조정에서 5,6모드도 사용", "Auto columns may use modes 5 and 6"),
    o!("SORT", 3, Kind::Choice(SORTS), "0", "파일정렬", "Sort", "파일 정렬 방식", "File sort order"),
    o!("SORTDESC", 3, Kind::Bool, "OFF", "내림차순", "Descending", "내림차순 정렬", "Sort descending"),
    o!("ARCHIVE", 3, Kind::Choice(ARCHIVES), "0", "압축파일", "Archive", "압축파일 만들때의 기본 방식", "Default archive type"),
    o!("ESC", 3, Kind::Choice(ESC_FN), "0", "Esc키", "Esc key", "Esc키에 배정할 기능 (종료는 Alt-X)", "Function of the Esc key (Alt-X quits)"),
    o!("HORILINE", 3, Kind::Bool, "ON", "가로선", "Horizontal line", "파일리스트의 가로구분선", "Horizontal lines in the file list"),
    o!("VERTLINE", 3, Kind::Bool, "ON", "세로선", "Vertical line", "파일리스트의 세로구분선", "Vertical lines in the file list"),
    o!("AUTOLINE", 3, Kind::Bool, "ON", "세로크기 자동", "Auto line", "파일리스트 세로크기 자동조정 (끄면 고정)", "Fit the file list height automatically"),
    o!("SCROLLBAR", 3, Kind::Bool, "ON", "스크롤 막대", "Scroll bar", "파일리스트 오른쪽의 스크롤 막대", "Scroll bar right of the file list"),
    o!("LOWER", 3, Kind::Bool, "OFF", "소문자", "Lowercase", "파일명을 소문자로 표시", "Show file names in lowercase"),
    o!("ATTR", 3, Kind::Bool, "OFF", "속성표시", "Attributes", "속성 표시 (숨은파일 포함)", "Show attributes (with hidden files)"),
    o!("HIDDEN", 3, Kind::Bool, "OFF", "숨은파일", "Hidden files", "숨은 파일 포함", "Include hidden files"),
    o!("EXEONLY", 3, Kind::Bool, "OFF", "실행파일만", "Exec files only", "실행파일과 디렉토리만 표시", "Only executables and directories"),
    o!("DEL_YES", 3, Kind::Bool, "OFF", "삭제초기값 예", "Delete default Yes", "삭제 확인상자의 초기 선택이 '예'", "Delete prompt starts on Yes"),
    o!("FULLMEM", 3, Kind::Bool, "ON", "풀메모리", "Full memory", "메모리 점유없이 실행 (현대 시스템에서는 같음)", "Batch-style run (same on modern systems)"),
    o!("DESCVIEW", 3, Kind::Choice(DESC_MODES), "1", "주석보기", "Descriptions", "주석 보기 (켜짐/자동/꺼짐)", "Show descriptions (on/auto/off)"),
    o!("DIREMUL", 3, Kind::Bool, "ON", "디렉토리재현", "Dir emulation", "압축파일 보기에서 디렉토리 재현", "Show archive folders as directories"),
    o!("INSDEL", 3, Kind::Bool, "OFF", "Ins/Del 차별화", "Ins/Del single", "Ins/Del은 선택막대의 파일 하나만", "Ins/Del act on the bar file only"),
    o!("SWAPDEL", 3, Kind::Bool, "OFF", "@D/Del 맞바꿈", "Swap @D/Del", "Alt-C/Alt-D와 Ins/Del 기능을 맞바꿈", "Swap Alt-C/Alt-D with Ins/Del"),
    o!("DIRDEL", 4, Kind::Bool, "ON", "디렉토리지움", "Delete dirs", "디렉토리 삭제 허용 (빈 디렉토리는 항상)", "Allow deleting directories (empty ones always)"),
    o!("ROOTDEL", 4, Kind::Bool, "OFF", "루트 삭제가능", "Root delete", "루트에서 삭제 명령 허용", "Allow delete on a root directory"),
    o!("DELKEY", 4, Kind::Bool, "ON", "DEL키 사용", "Use DEL key", "Del키를 삭제 명령키로 사용", "Del key deletes"),
    o!("MCDSORT", 4, Kind::Bool, "OFF", "MCD 정렬", "MCD sort", "MCD 디렉토리를 이름순 정렬", "Sort MCD tree by name"),
    o!("MCDSTART", 4, Kind::Bool, "OFF", "MCD시작", "Start with MCD", "시작할 때 MCD부터", "Start in MCD"),
    o!("MCDQUIT", 4, Kind::Bool, "OFF", "MCD후 종료", "Quit after MCD", "MCD만 사용한 후 도스로", "Quit after MCD"),
    o!("FDDPAUSE", 4, Kind::Bool, "OFF", "FDD 정지", "FDD pause", "디렉토리 없는 드라이브에서도 멈춤", "Stop on drives without directories"),
    o!("SAVETIME", 5, Kind::Num(0, 99), "5", "화면보호시간", "Saver time", "화면보호 대기시간 (분, 0=안함)", "Screen saver delay (minutes, 0=off)"),
    o!("HANGUL", 5, Kind::Choice(HANGUL), "0", "한글코드", "Hangul code", "영문이면 영문 메시지", "English shows English messages"),
    o!("LINES", 5, Kind::Choice(LINES), "0", "화면크기", "Screen lines", "화면 줄 수 (기본값=터미널 크기)", "Screen lines (default = terminal size)"),
    o!("HISDIR", 5, Kind::Choice(HISDIR), "1", "M.HIS저장", "M.HIS place", "히스토리와 트리파일을 저장할 곳", "Where history and tree files go"),
    o!("LAN", 5, Kind::Str, "", "LAN드라이브", "LAN drives", "재검색하지 않을 드라이브 범위 (E-J)", "Drives not rescanned (E-J)"),
    o!("24HOUR", 5, Kind::Bool, "ON", "24시간제", "24 hour", "시간을 24시간제로", "24-hour clock"),
    o!("ZOOMBOX", 5, Kind::Bool, "OFF", "느린상자열기", "Zoom box", "상자가 천천히 열림", "Boxes open slowly"),
    o!("LOGO", 5, Kind::Bool, "ON", "로고상자표시", "Logo box", "시작할 때 로고상자", "Logo box at start"),
    o!("MOUSE", 5, Kind::Bool, "ON", "마우스사용", "Mouse", "마우스 사용", "Use the mouse"),
    o!("LEFTMOUSE", 5, Kind::Bool, "OFF", "왼손마우스", "Left-hand mouse", "좌우 버튼을 맞바꿈", "Swap mouse buttons"),
    o!("QUICKMOUSE", 5, Kind::Bool, "ON", "빠른초기화", "Quick mouse", "빠른 마우스 초기화", "Quick mouse init"),
    o!("FASTKEY", 5, Kind::Bool, "OFF", "빠른 키입력", "Fast keys", "키입력 속도를 최대로", "Fastest key repeat"),
    o!("EXTSEARCH", 5, Kind::Bool, "OFF", "확장빨리찾기", "Ext speed search", "같은 첫글자를 다시 누르면 다음 파일", "Same first letter jumps to next match"),
    o!("MONO", 5, Kind::Bool, "OFF", "흑백색깔사용", "Monochrome", "지정 색깔 무시하고 흑백", "Ignore colours, use mono"),
    o!("DRIVETYPE", 5, Kind::Bool, "ON", "드라이브종류", "Drive type", "드라이브명 뒤에 종류 표시", "Show drive type after the drive"),
    o!("CACHEFLUSH", 5, Kind::Bool, "OFF", "캐쉬버퍼저장", "Flush cache", "파일작업 후 디스크 버퍼 기록", "Flush disk buffers after file jobs"),
    o!("LFN", 5, Kind::Bool, "ON", "긴파일명지원", "Long names", "긴파일명 지원", "Long file name support"),
];

pub fn opt(key: &str) -> Option<&'static Opt> {
    OPTIONS.iter().find(|o| o.key == key)
}

/// Function keys in MSET order: F2-F9, Ctrl-F1..F10, Alt-F1..F10 (28 keys).
pub fn fkey_names() -> Vec<String> {
    let mut v: Vec<String> = (2..=9).map(|i| format!("F{}", i)).collect();
    v.extend((1..=10).map(|i| format!("CF{}", i)));
    v.extend((1..=10).map(|i| format!("AF{}", i)));
    v
}

pub fn fkey_label(k: &str) -> String {
    if let Some(n) = k.strip_prefix("CF") {
        format!("Ctrl-F{}", n)
    } else if let Some(n) = k.strip_prefix("AF") {
        format!("Alt-F{}", n)
    } else {
        k.to_string()
    }
}

#[derive(Clone)]
pub struct Config {
    pub path: PathBuf,
    pub opts: BTreeMap<String, String>,
    pub fkeys: BTreeMap<String, String>,
    pub ext: Vec<(String, String)>,
    pub qcd: [String; 10],
    pub colors: Colors,
    pub dirty: bool,
}

impl Config {
    pub fn default_path() -> PathBuf {
        std::env::var("M_CFG").map(PathBuf::from).unwrap_or_else(|_| crate::term::data_dir().join("M.CFG"))
    }

    pub fn defaults(path: PathBuf) -> Config {
        let mut fkeys = BTreeMap::new();
        // Sample function keys like the shipped configuration's: viewer, file find, format, disk copy.
        fkeys.insert("F8".into(), "%Mvv !.!".into());
        fkeys.insert("F9".into(), "%Mmff *.*%P".into());
        let ext = vec![
            ("TXT".into(), "%Mvv !.!".into()),
            ("DOC".into(), "%Mvv !.!".into()),
            ("HLP".into(), "%Mvv !.!".into()),
        ];
        Config { path, opts: BTreeMap::new(), fkeys, ext, qcd: Default::default(), colors: Colors::default(), dirty: false }
    }

    pub fn load() -> Config {
        Self::load_from(&Self::default_path())
    }

    pub fn load_from(path: &Path) -> Config {
        let mut c = Config::defaults(path.to_path_buf());
        let Ok(text) = std::fs::read_to_string(path) else { return c };
        c.ext.clear();
        c.fkeys.clear();
        let mut sec = String::from("option");
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                sec = line[1..line.len() - 1].to_ascii_lowercase();
                continue;
            }
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim().to_ascii_uppercase(), v.trim().to_string());
            match sec.as_str() {
                "function" => {
                    c.fkeys.insert(k, v);
                }
                "extension" => c.ext.push((k, v)),
                "qcd" => {
                    if let Ok(i) = k.parse::<usize>() {
                        if (1..=10).contains(&i) {
                            c.qcd[i - 1] = v;
                        }
                    }
                }
                "color" => {
                    if let Some(i) = ITEM_NAMES.iter().position(|(_, en)| en.replace(' ', "").eq_ignore_ascii_case(&k)) {
                        if let Ok(a) = u8::from_str_radix(&v, 16) {
                            c.colors.items[i] = a;
                        }
                    }
                }
                "extcolor" => {
                    if let Ok(a) = u8::from_str_radix(&v, 16) {
                        if let Some(e) = c.colors.ext.iter_mut().find(|(e, _)| e.eq_ignore_ascii_case(&k)) {
                            e.1 = a;
                        } else {
                            c.colors.ext.push((k, a));
                        }
                    }
                }
                _ => {
                    c.opts.insert(k, v);
                }
            }
        }
        c
    }

    pub fn save(&self) -> std::io::Result<()> {
        let mut s = String::from("; M-dir configuration file (M.CFG). Edit with MSET.\n[option]\n");
        for o in OPTIONS {
            s += &format!("{}={}\n", o.key, self.get(o.key));
        }
        s += "\n[function]\n";
        for k in fkey_names() {
            if let Some(v) = self.fkeys.get(&k) {
                if !v.is_empty() {
                    s += &format!("{}={}\n", k, v);
                }
            }
        }
        s += "\n[extension]\n";
        for (e, v) in &self.ext {
            s += &format!("{}={}\n", e, v);
        }
        s += "\n[qcd]\n";
        for (i, d) in self.qcd.iter().enumerate() {
            if !d.is_empty() {
                s += &format!("{}={}\n", i + 1, d);
            }
        }
        s += "\n[color]\n";
        for (i, (_, en)) in ITEM_NAMES.iter().enumerate() {
            s += &format!("{}={:02X}\n", en.replace(' ', ""), self.colors.items[i]);
        }
        s += "\n[extcolor]\n";
        for (e, a) in &self.colors.ext {
            s += &format!("{}={:02X}\n", e, a);
        }
        if let Some(d) = self.path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&self.path, s)
    }

    pub fn get(&self, key: &str) -> String {
        self.opts.get(key).cloned().unwrap_or_else(|| opt(key).map(|o| o.default.to_string()).unwrap_or_default())
    }
    pub fn flag(&self, key: &str) -> bool {
        matches!(self.get(key).to_ascii_uppercase().as_str(), "ON" | "1" | "YES" | "O" | "TRUE")
    }
    pub fn num(&self, key: &str) -> i64 {
        self.get(key).trim().parse().unwrap_or(0)
    }
    pub fn set(&mut self, key: &str, v: &str) {
        self.opts.insert(key.to_string(), v.to_string());
        self.dirty = true;
    }
    pub fn set_flag(&mut self, key: &str, on: bool) {
        self.set(key, if on { "ON" } else { "OFF" })
    }

    pub fn ext_prog(&self, ext: &str) -> Option<&str> {
        self.ext.iter().find(|(e, _)| e.eq_ignore_ascii_case(ext)).map(|(_, v)| v.as_str())
    }

    pub fn apply_language(&self) {
        let h = self.num("HANGUL");
        if h == 3 {
            crate::text::set_korean(false);
        } else if std::env::var("MDIR_LANG").is_err() {
            crate::text::set_korean(true);
        }
    }
}

/// Program text with variables replaced, plus the option variables it carried.
#[derive(Debug, Default, Clone)]
pub struct Expanded {
    pub cmd: String,
    pub pause: bool,
    pub prompt: Option<usize>,
    pub in_prog_dir: bool,
}

pub struct VarCtx<'a> {
    pub dir: &'a Path,
    /// File under the bar (None when the bar is on a drive name).
    pub file: Option<&'a str>,
    pub drive: Option<&'a str>,
    pub mdir: &'a Path,
    pub qcd: &'a [String; 10],
    pub cfg: &'a Config,
}

fn q(s: &str) -> String {
    if s.contains(' ') && !s.starts_with('"') {
        format!("\"{}\"", s)
    } else {
        s.to_string()
    }
}

fn with_sep(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    if s.ends_with(std::path::MAIN_SEPARATOR) {
        s
    } else {
        format!("{}{}", s, std::path::MAIN_SEPARATOR)
    }
}

/// Replaces MdirIII's variables (!.! %F !, ,! !\ !: %A %M %1..%0 %[EXT] %; %L) and strips the option
/// variables (%B %N %P %$ %%), reporting them. Errors on a looping %[EXT] definition.
pub fn expand(prog: &str, c: &VarCtx) -> Result<Expanded, String> {
    let mut out = Expanded::default();
    let mut s = String::new();
    expand_into(prog, c, &mut s, &mut out, 0)?;
    out.cmd = s.trim().to_string();
    Ok(out)
}

fn expand_into(prog: &str, c: &VarCtx, s: &mut String, out: &mut Expanded, depth: u32) -> Result<(), String> {
    if depth > 8 {
        return Err(t("환경설정이 잘못되었습니다", "Bad program definition. Run MSET.EXE").into());
    }
    let file = c.file.unwrap_or("");
    let (base, ext) = crate::text::split_ext(file);
    let drive_text = c.drive.unwrap_or("");
    let full = c.dir.join(file).to_string_lossy().to_string();
    let ch: Vec<char> = prog.chars().collect();
    let mut i = 0;
    let at = |i: usize, p: &str| -> bool { ch[i..].iter().take(p.chars().count()).copied().eq(p.chars()) };
    while i < ch.len() {
        if at(i, "!\\!.!") {
            *s += &q(if c.file.is_some() { &full } else { drive_text });
            i += 5;
        } else if at(i, "!,.,!") {
            *s += &q(&if ext.is_empty() { format!("{}.", base) } else { file.to_string() });
            i += 5;
        } else if at(i, "!.!") {
            *s += &q(if c.file.is_some() { file } else { drive_text });
            i += 3;
        } else if at(i, "!\\") {
            *s += &q(&with_sep(c.dir));
            i += 2;
        } else if at(i, "!:") {
            let d = c.dir.to_string_lossy();
            *s += if cfg!(windows) && d.len() >= 2 { &d[..2] } else { "/" };
            i += 2;
        } else if at(i, "!,") {
            *s += &q(if c.file.is_some() { base } else { drive_text });
            i += 2;
        } else if at(i, ",!") {
            *s += if c.file.is_some() { ext } else { drive_text };
            i += 2;
        } else if ch[i] == '%' && i + 1 < ch.len() {
            let n = ch[i + 1];
            match n.to_ascii_uppercase() {
                'F' | 'L' => *s += &q(if c.file.is_some() { file } else { drive_text }),
                'A' => {
                    let name = c.dir.file_name().map(|f| f.to_string_lossy().to_string());
                    *s += &match name {
                        Some(n) => crate::text::split_ext(&n).0.to_string(),
                        None => "ROOT".into(),
                    }
                }
                'M' => *s += &q(&with_sep(c.mdir)),
                'D' | 'R' => {
                    *s += &q(&with_sep(c.dir));
                    out.prompt.get_or_insert(s.chars().count());
                }
                '0'..='9' => {
                    let k = if n == '0' { 9 } else { n as usize - '1' as usize };
                    *s += &q(&c.qcd[k]);
                }
                ';' => *s += ";",
                'B' | 'N' => {}
                'P' => {
                    out.prompt = Some(s.chars().count());
                }
                '$' => out.in_prog_dir = true,
                '%' => out.pause = true,
                '[' => {
                    let end = ch[i + 2..].iter().position(|&c| c == ']').map(|p| p + i + 2);
                    if let Some(e) = end {
                        let key: String = ch[i + 2..e].iter().collect();
                        let def = c.cfg.ext_prog(&key).unwrap_or("").to_string();
                        expand_into(&def, c, s, out, depth + 1)?;
                        i = e + 1;
                        continue;
                    }
                    s.push('%');
                    i += 1;
                    continue;
                }
                _ => {
                    s.push('%');
                    i += 1;
                    continue;
                }
            }
            i += 2;
        } else {
            s.push(ch[i]);
            i += 1;
        }
    }
    Ok(())
}

/// Splits a definition into its alternative programs on ';' (but not "%;").
pub fn alternatives(prog: &str) -> Vec<String> {
    let mut v = vec![];
    let mut cur = String::new();
    let mut it = prog.chars().peekable();
    while let Some(c) = it.next() {
        if c == '%' && it.peek() == Some(&';') {
            cur.push_str("%;");
            it.next();
        } else if c == ';' {
            v.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    v.push(cur);
    v.into_iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).take(20).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vars() {
        let cfg = {
            let mut c = Config::defaults(PathBuf::from("/tmp/x.cfg"));
            c.ext = vec![("PCX".into(), "qpv !.!".into()), ("GIF".into(), "%[PCX]".into()), ("A".into(), "%[B]".into()), ("B".into(), "%[A]".into())];
            c
        };
        let qcd: [String; 10] = Default::default();
        let ctx = VarCtx { dir: Path::new("/home/u/doc"), file: Some("TEST.DOC"), drive: None, mdir: Path::new("/opt/m"), qcd: &qcd, cfg: &cfg };
        assert_eq!(expand("hwp !.!", &ctx).unwrap().cmd, "hwp TEST.DOC");
        assert_eq!(expand("x !, ,!", &ctx).unwrap().cmd, "x TEST DOC");
        assert_eq!(expand("%[GIF]", &ctx).unwrap().cmd, "qpv TEST.DOC");
        assert!(expand("%[A]", &ctx).is_err());
        let e = expand("dir%%", &ctx).unwrap();
        assert!(e.pause);
        assert_eq!(e.cmd, "dir");
        // the directory and file are joined with the platform's separator
        assert_eq!(expand("cat !\\!.!", &ctx).unwrap().cmd, format!("cat /home/u/doc{}TEST.DOC", std::path::MAIN_SEPARATOR));
        assert_eq!(alternatives("a;b %; c;;d"), vec!["a", "b %; c", "d"]);
    }
}
