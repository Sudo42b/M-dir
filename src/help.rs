//! Built-in F1 help, written for this project. If the person keeps their own M.HLP / M-K.HLP in the
//! data folder, `App::open_help` shows that instead.

pub const KO: &str = "\
 M-dir 도움말                                            (Esc: 닫기)

 [이동]
   ↑ ↓ ← → PgUp PgDn Home End   막대 이동
   글자키                        빨리찾기 (그 글자로 시작하는 파일로)
   Enter                         실행 / 디렉토리로 들어가기
   .  \\                          상위 디렉토리 / 루트
   Ctrl-H, Ctrl-BkSp             이전 디렉토리
   F10                           디렉토리 관리기(MCD)
   F11, Shift-F1..F10            QCD (자주 가는 디렉토리)

 [선택]
   Space                         선택/해제
   Alt-U  *                      모두 선택 / 선택 뒤집기
   +  -                          무리 선택 / 무리 해제
   Alt-W                         작업 디렉토리 / Ctrl-W 닫기

 [파일 작업]
   Alt-C  Ins                    복사
   Alt-M                         이동
   Alt-D  Del                    삭제
   Alt-R                         이름 바꾸기
   Alt-K                         디렉토리 만들기
   Alt-V                         보기 (VV)
   Alt-I                         파일 찾기 (MFF)
   Alt-P                         인쇄
   Ctrl-Z                        속성 바꾸기
   Alt-Enter                     주석 달기

 [압축파일]
   Alt-`                         압축파일 안으로 들어가기
   Ctrl-A  Ctrl-M  Ctrl-X        압축하기 / 압축하며 옮기기 / 풀기

 [화면]
   Alt-0..6                      열 개수 (0: 자동)
   Alt-N E S T B Q O             정렬: 이름 확장자 크기 시간 색 실행 없음
   Alt--                         정렬 방향
   Alt-A  Alt-Z  Alt-G  Alt-F    속성 보기 / 숨김파일 / 실행파일만 / 걸러보기
   Alt-L                         소문자로 보기
   Alt-=  Ctrl-E                 줄 수 자동 / 줄 수 바꾸기

 [기타]
   F12  Ctrl-Ins                 풀다운 메뉴
   Ctrl-F12                      환경설정 (MSET)
   /  ?                          도스(셸) 명령
   Ctrl-C  Ctrl-F                디스크 이미지 복사 / 포맷
   Ctrl-S                        화면보호
   Alt-X                         끝내기
";

pub const EN: &str = "\
 M-dir help                                              (Esc: close)

 [Moving]
   Arrows PgUp PgDn Home End     move the bar
   Letter keys                   speed search (jump to a file starting with it)
   Enter                         run / enter a directory
   .  \\                          parent / root directory
   Ctrl-H, Ctrl-BkSp             previous directory
   F10                           directory manager (MCD)
   F11, Shift-F1..F10            QCD (favourite directories)

 [Selecting]
   Space                         tag / untag
   Alt-U  *                      tag all / invert tags
   +  -                          tag group / untag group
   Alt-W                         work directory / Ctrl-W closes it

 [Files]
   Alt-C  Ins                    copy
   Alt-M                         move
   Alt-D  Del                    delete
   Alt-R                         rename
   Alt-K                         make directory
   Alt-V                         view (VV)
   Alt-I                         find files (MFF)
   Alt-P                         print
   Ctrl-Z                        change attributes
   Alt-Enter                     describe

 [Archives]
   Alt-`                         browse inside an archive
   Ctrl-A  Ctrl-M  Ctrl-X        add / move into / extract

 [Screen]
   Alt-0..6                      number of columns (0: auto)
   Alt-N E S T B Q O             sort: name ext size time colour exec none
   Alt--                         sort direction
   Alt-A  Alt-Z  Alt-G  Alt-F    attributes / hidden / executables only / filter
   Alt-L                         lower-case names
   Alt-=  Ctrl-E                 fit lines / change line count

 [Other]
   F12  Ctrl-Ins                 pull-down menu
   Ctrl-F12                      settings (MSET)
   /  ?                          shell command
   Ctrl-C  Ctrl-F                disk image copy / format
   Ctrl-S                        screen saver
   Alt-X                         quit
";
