//! Every command of the main screen, the keys that start them and the pull-down menu that lists them.
use crate::list::Sort;
use mdir_core::modal::{alt, ctrl, shift};
use mdir_core::ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventState};

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    About, Help, ViewEnv, Park, VolLabel, Format, DiskCopy, Saver, DosCmd, DosCmdSwap, Quit,
    Run, RunParam, RunSwap, RunSwapParam, RunQuit, RunQuitParam, DosScreen,
    Copy, Move, Rename, Delete, Attrib, Describe, LfnDesc, View, Print, Find,
    SelectAll, Invert, SelectGroup, DeselectGroup, SelectExt, Tag,
    Mcd, Qcd, Mkdir, Parent, Root, Prev, Work, WorkClose,
    ArcView, ArcAdd, ArcMove, ArcExtract, DirEmul,
    ExeOnly, ViewAttr, Hidden, Filter, Lower, Lines, AutoLine, HLine, Setup,
    GraphicChar, CycleColumns, CycleSort, Columns(u8), SortBy(Sort), SortDir,
    CopyIns, DeleteDel, Fkey(String), Qcd1(usize), Menu,
}

/// Main-screen key bindings (M.HLP / M.DOC). Plain letters and Shift-letters are handled by the caller
/// (speed search and drive change).
pub fn key_act(k: &KeyEvent) -> Option<Act> {
    use Act::*;
    let (c, a, s) = (ctrl(k), alt(k), shift(k));
    let keypad = k.state.contains(KeyEventState::KEYPAD);
    Some(match k.code {
        KeyCode::F(1) if s => Qcd1(0),
        KeyCode::F(n) if s && n <= 10 => Qcd1(n as usize - 1),
        KeyCode::F(1) if !c && !a => Help,
        KeyCode::F(10) if !c && !a => Mcd,
        KeyCode::F(11) => Qcd,
        KeyCode::F(12) if c => Setup,
        KeyCode::F(12) => Menu,
        KeyCode::F(n) if (2..=9).contains(&n) && !c && !a => Fkey(format!("F{}", n)),
        KeyCode::F(n) if n <= 10 && c => Fkey(format!("CF{}", n)),
        KeyCode::F(n) if n <= 10 && a => Fkey(format!("AF{}", n)),
        KeyCode::Insert if c => Menu,
        KeyCode::Insert => CopyIns,
        KeyCode::Delete => DeleteDel,
        KeyCode::Enter if a => Describe,
        KeyCode::Enter if c => RunParam,
        KeyCode::Enter => Run,
        KeyCode::Tab => RunSwap,
        KeyCode::BackTab => RunSwapParam,
        KeyCode::Char(' ') if a => RunQuit,
        KeyCode::Char(' ') if c => RunQuitParam,
        KeyCode::Char(' ') => Tag,
        KeyCode::Backspace if c => Prev,
        KeyCode::Char('h') if c => Prev,
        KeyCode::Char('/') if keypad => SelectExt,
        KeyCode::Char('/') => DosCmd,
        KeyCode::Char('?') => DosCmdSwap,
        KeyCode::Char('*') => Invert,
        KeyCode::Char('+') if !a => SelectGroup,
        KeyCode::Char('-') if a => SortDir,
        KeyCode::Char('-') => DeselectGroup,
        KeyCode::Char('.') if !a => Parent,
        KeyCode::Char('\\') if a => HLine,
        KeyCode::Char('\\') => Root,
        KeyCode::Char('=') if a => AutoLine,
        KeyCode::Char('`') if a => DirEmul,
        KeyCode::Char('\'') if a => DirEmul,
        KeyCode::Char(ch) if a => match ch.to_ascii_lowercase() {
            'x' => Quit,
            'c' => Copy,
            'm' => Move,
            'd' => Delete,
            'r' => Rename,
            'k' => Mkdir,
            'p' => Print,
            'v' => View,
            'i' => Find,
            'u' => SelectAll,
            'w' => Work,
            'a' => ViewAttr,
            'z' => Hidden,
            'f' => Filter,
            'g' => ExeOnly,
            'l' => Lower,
            'n' => SortBy(Sort::Name),
            'e' => SortBy(Sort::Ext),
            's' => SortBy(Sort::Size),
            't' => SortBy(Sort::Time),
            'b' => SortBy(Sort::Color),
            'q' => SortBy(Sort::Exec),
            'o' => SortBy(Sort::None),
            d @ '0'..='6' => Columns(d as u8 - b'0'),
            _ => return None,
        },
        KeyCode::Char(ch) if c => match ch.to_ascii_lowercase() {
            'z' => Attrib,
            'q' => LfnDesc,
            'c' => DiskCopy,
            'f' => Format,
            'v' => VolLabel,
            'p' => Park,
            'a' => ArcAdd,
            'm' => ArcMove,
            'x' => ArcExtract,
            'e' => Lines,
            't' => DosScreen,
            's' => Saver,
            'w' => WorkClose,
            _ => return None,
        },
        _ => return None,
    })
}

pub struct Item {
    pub ko: &'static str,
    pub en: &'static str,
    pub act: Act,
}

pub struct MenuDef {
    pub ko: &'static str,
    pub en: &'static str,
    pub items: Vec<Item>,
}

macro_rules! it {
    ($ko:expr, $en:expr, $a:expr) => {
        Item { ko: $ko, en: $en, act: $a }
    };
}

/// The six pull-down menus, with item texts as M.EXE shows them.
pub fn menus() -> Vec<MenuDef> {
    use Act::*;
    vec![
        MenuDef { ko: " ^메^인 ", en: " ^M^dir ", items: vec![
            it!(" 제작번호            ", " About               ", About),
            it!(" 도움말              ^F1^ ", " Help                ^F1^ ", Help),
            it!(" 환경설정 상태보기      ", " View environment       ", ViewEnv),
            it!(" 파킹            ^Ctrl-P^ ", " Parking         ^Ctrl-P^ ", Park),
            it!(" 볼륨명바꾸기    ^Ctrl-V^ ", " Volume label    ^Ctrl-V^ ", VolLabel),
            it!(" 디스크포맷      ^Ctrl-F^ ", " Format          ^Ctrl-F^ ", Format),
            it!(" 디스크복사      ^Ctrl-C^ ", " diskCopy        ^Ctrl-C^ ", DiskCopy),
            it!(" 화면보호        ^Ctrl-S^ ", " Screen saver    ^Ctrl-S^ ", Saver),
            it!(" 도스명령             ^/^ ", " Dos command          ^/^ ", DosCmd),
            it!(" 도스명령(스와핑)     ^?^ ", " Dos command(swap)    ^?^ ", DosCmdSwap),
            it!(" 도스로 나감      ^Alt-X^ ", " Quit to dos      ^Alt-X^ ", Quit),
        ]},
        MenuDef { ko: " ^실^행 ", en: " ^R^un ", items: vec![
            it!(" 실행               ^Enter^ ", " Run                ^Enter^ ", Run),
            it!(" 실행(옵션)       ^C-Enter^ ", " Run(param)       ^C-Enter^ ", RunParam),
            it!(" 실행(스와핑)         ^Tab^ ", " Run(swap)            ^Tab^ ", RunSwap),
            it!(" 실행(스와핑,옵션)  ^S-Tab^ ", " Run(swap,param)    ^S-Tab^ ", RunSwapParam),
            it!(" 실행(종료)       ^A-SPACE^ ", " Run(quit)        ^A-SPACE^ ", RunQuit),
            it!(" 실행(종료,옵션)  ^C-SPACE^ ", " Run(quit,param)  ^C-SPACE^ ", RunQuitParam),
            it!(" 도스화면 보기     ^Ctrl-T^ ", " View dos screen   ^Ctrl-T^ ", DosScreen),
        ]},
        MenuDef { ko: " ^파^일 ", en: " ^F^ile ", items: vec![
            it!(" 복사           ^Alt-C^ ", " Copy           ^Alt-C^ ", Copy),
            it!(" 이동           ^Alt-M^ ", " Move           ^Alt-M^ ", Move),
            it!(" 이름바꾸기     ^Alt-R^ ", " Rename         ^Alt-R^ ", Rename),
            it!(" 지우기         ^Alt-D^ ", " Delete         ^Alt-D^ ", Delete),
            it!(" 속성바꾸기    ^Ctrl-Z^ ", " Attributes    ^Ctrl-Z^ ", Attrib),
            it!(" 주석입력      ^@Enter^ ", " Describe      ^@Enter^ ", Describe),
            it!(" 긴파일->주석  ^Ctrl-Q^ ", " LFN to desc   ^Ctrl-Q^ ", LfnDesc),
            it!(" 보기           ^ALT-V^ ", " View           ^ALT-V^ ", View),
            it!(" 인쇄           ^ALT-P^ ", " Print          ^ALT-P^ ", Print),
            it!(" 파일찾기       ^ALT-I^ ", " FileFind       ^ALT-I^ ", Find),
            it!(" 전부선택       ^Alt-U^ ", " Select all     ^Alt-U^ ", SelectAll),
            it!(" 선택반전           ^*^ ", " Invert select      ^*^ ", Invert),
            it!(" 파일명으로 선택    ^+^ ", " Select   group     ^+^ ", SelectGroup),
            it!(" 파일명으로 취소    ^-^ ", " Deselect group     ^-^ ", DeselectGroup),
        ]},
        MenuDef { ko: " ^디^렉토리 ", en: " ^D^irectory ", items: vec![
            it!(" 디렉토리관리기     ^F10^ ", " MCD                ^F10^ ", Mcd),
            it!(" 디렉토리빨리바꾸기 ^F11^ ", " QCD                ^F11^ ", Qcd),
            it!(" 만들기           ^Alt-K^ ", " maKe             ^Alt-K^ ", Mkdir),
            it!(" 이름바꾸기       ^Alt-R^ ", " Rename           ^Alt-R^ ", Rename),
            it!(" 지우기           ^Alt-D^ ", " Delete           ^Alt-D^ ", Delete),
            it!(" 복사             ^Alt-C^ ", " Copy             ^Alt-C^ ", Copy),
            it!(" 이동             ^Alt-M^ ", " Move             ^Alt-M^ ", Move),
            it!(" 부모디렉토리로       ^.^ ", " To parent            ^.^ ", Parent),
            it!(" 루트디렉토리로       ^\\^ ", " To root              ^\\^ ", Root),
            it!(" 이전디렉토리로 ^Ctrl-BS^ ", " To previous    ^Ctrl-BS^ ", Prev),
            it!(" 작업디렉토리     ^Alt-W^ ", " Work directory   ^Alt-W^ ", Work),
            it!(" 작업 닫기       ^Ctrl-W^ ", " Close           ^Ctrl-W^ ", WorkClose),
        ]},
        MenuDef { ko: " ^압^축파일 ", en: " ^A^rchive ", items: vec![
            it!(" 보기         ^Enter^ ", " View         ^Enter^ ", ArcView),
            it!(" 압축        ^Ctrl-A^ ", " Add         ^Ctrl-A^ ", ArcAdd),
            it!(" 압축(이동)  ^Ctrl-M^ ", " add & Move  ^Ctrl-M^ ", ArcMove),
            it!(" 풀기        ^Ctrl-X^ ", " eXtract     ^Ctrl-X^ ", ArcExtract),
        ]},
        MenuDef { ko: " ^선^택사항 ", en: " ^O^ption ", items: vec![
            it!(" 실행파일만       ^Alt-G^ ", " Exec files only  ^Alt-G^ ", ExeOnly),
            it!(" 속성보기         ^Alt-A^ ", " View attribute   ^Alt-A^ ", ViewAttr),
            it!(" 숨은파일포함     ^Alt-Z^ ", " Show hidden file ^Alt-Z^ ", Hidden),
            it!(" 필터             ^Alt-F^ ", " Filter           ^Alt-F^ ", Filter),
            it!(" 소문자로         ^Alt-L^ ", " Lowercase        ^Alt-L^ ", Lower),
            it!(" 28,43/50 줄     ^Ctrl-E^ ", " 28,43/50 line   ^Ctrl-E^ ", Lines),
            it!(" 세로크기고정     ^Alt-=^ ", " Auto line        ^Alt-=^ ", AutoLine),
            it!(" 가로선           ^Alt-\\^ ", " Horizontal line  ^Alt-\\^ ", HLine),
            it!(" 환경설정      ^Ctrl-F12^ ", " Setup(MSET)   ^Ctrl-F12^ ", Setup),
            it!(" 그래픽문자   : ", " Graphic Char : ", GraphicChar),
            it!(" 컬럼수       :    ", " Column       :    ", CycleColumns),
            it!(" 정렬방식     :   ", " Sort         :   ", CycleSort),
        ]},
    ]
}

/// Bottom-line hints shown while Alt or Ctrl is held.
pub fn hints(ctrl: bool) -> &'static str {
    use mdir_core::text::t;
    if ctrl {
        t(" ^Z^-속성바꾸기 ^A^-압축(^M^-이동)/^X^-풀기 ^P^-파킹 ^T^-도스화면 ^L^-다음찾기 ^Enter^-실행 ^E^GA",
          " ^Z^-change attr ^A^dd(^M^ove)/e^X^tract ^P^ark ^T^-last screen ^L^-Next search ^Enter^-Run ^E^GA")
    } else {
        t(" ^C^-복사 ^M^-이동 ^D^-삭제 ^R^-새이름 ^V^-보기 ^P^-인쇄 ^K^-디렉토리만들기 ^A^-속성보기 ^U^-선택",
          " ^C^opy ^M^ove ^D^el ^R^ename ^V^iew ^P^rint m^K^dir ^A^ttrib ^L^ower ^F^ilter ^Z^-Hidden ^G^-Exe ^U^-Tag")
    }
}
