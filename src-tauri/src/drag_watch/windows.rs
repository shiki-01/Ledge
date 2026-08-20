//! Windows向け「ドラッグ操作開始」ヒューリスティック検知実装（F-08, architecture.md 8.1章）。
//!
//! `WH_MOUSE_LL`（低レベルマウスフック）で「左ボタン押下→一定距離（DPI換算で既定8論理px相当）
//! 以上の移動」というパターンを検知し、ドラッグ操作の可能性が高いと判定してコールバックを呼ぶ。
//! OLEドラッグそのものは検知できない（Windowsにはグローバルなドラッグ開始通知APIが存在しない）
//! ため、あくまでヒューリスティックであり、ファイルを伴わない単なるドラッグ選択やウィンドウ移動
//! でも反応しうる。誤検知の実害は小さい（シェルフが表示されるだけ）という判断のもと許容する
//! （architecture.md 8.1章）。
//!
//! `#[cfg(target_os = "windows")]`配下のためWindows以外の開発環境（macOS/Linux等）では
//! コンパイル対象外であり、**実機（Windows）での動作確認は未実施**。構文的な妥当性と
//! ロジックの妥当性のレビューに留まる（architecture.md 7章）。開発環境がmacOSだった回では
//! `windows`クレート自体がターゲット限定依存のためフェッチされず、`cargo build`でこの
//! ファイルを検証することもできなかった。
//!
//! 簡略化した点（呼び出し元への報告事項）: architecture.md 8.1章は「一定時間（既定800ms）操作が
//! 無ければ自動的に表示状態を解除して**良い**」と許容的に書いており必須要件ではないため、
//! タイマーによる無操作タイムアウト解除は実装せず、「ボタンを離す」ことのみを終了条件とする
//! 簡易実装とした。
//!
//! ## エッジ近傍判定の追加について
//! 当初は「左ボタン押下→一定距離以上の移動」だけでシェルフを自動表示していたが、画面上のどこで
//! クリック&ドラッグしても（無関係な操作やウィンドウ移動でも）反応してしまい邪魔、という
//! ユーザー報告があった。対策として、設定済みのシェルフ表示端（`AppSettings.shelf_edge`）に近い
//! エリアに入った時だけ発火するよう変更した（`EdgeGeometry`, `in_edge_zone`）。ジオメトリの取得に
//! 失敗した場合（`edge: None`）は、安全側に倒し従来通り無条件で発火する。
//!
//! ## タイトルバードラッグ（ウィンドウ移動）の除外について
//! エッジ近傍判定を追加した後も、「ウィンドウを画面端に持っていくとまだシェルフが出る」という
//! ユーザー報告があった。これはウィンドウのタイトルバーを掴んでエッジ付近までドラッグ移動する
//! 操作が、上記のエッジ近傍判定を素通りしてしまうために起きる。macOS版では
//! `NSPasteboard(name: .drag)`を使い「本当にコンテンツをドラッグしているか」を直接判定できるが、
//! Windowsには同等のグローバルなドラッグ内容参照手段が無いため、代わりに「左ボタン押下位置が
//! ウィンドウのタイトルバー（キャプション領域）だったら、ウィンドウ移動の開始とみなして除外する」
//! というヒューリスティックを追加した（`is_title_bar_hit`、`WM_NCHITTEST`/`HTCAPTION`を使用）。
//!
//! `WM_NCHITTEST`の判定に使う`SendMessageW`は対象ウィンドウのメッセージループへ同期送信し
//! 応答を待つブロッキング呼び出しであり、対象アプリの応答が遅い場合に時間がかかりうる。そのため
//! `handle_event`/`hook_proc`（`WH_MOUSE_LL`フックプロシージャの呼び出し系列）の中では絶対に
//! 呼び出さず、`invoke_on_start`（メッセージループスレッドの通常のディスパッチ処理内、フック
//! プロシージャの外）からのみ呼び出す設計とした（本ファイル内の「低レベルフックタイムアウト」に
//! 関する既存の注意書きを参照）。
//!
//! 実機（Windows）での動作確認は未実施であることに加え、このmacOS開発環境ではこのファイルは
//! そもそもコンパイル対象外（`#[cfg(target_os = "windows")]`）であり、`cargo build`/`cargo check`
//! による検証も一切できていない。型・API名の妥当性はvendoringされた`windows`クレート0.61.3の
//! ソース（`~/.cargo/registry/src/.../windows-0.61.3/src/Windows/Win32/UI/WindowsAndMessaging/
//! mod.rs`および`Foundation/mod.rs`）を直接読んで照合したが、実行時の挙動検証はできていない
//! （呼び出し元への報告事項）。

use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SendMessageW,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WindowFromPoint, HHOOK, HTCAPTION,
    MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCHITTEST,
    WM_QUIT,
};

use crate::error::ShelfError;

use super::{DragWatcher, EdgeGeometry};

/// ドラッグ開始とみなす移動距離（論理px換算、architecture.md 8.1章の既定値）。
const DRAG_THRESHOLD_LOGICAL_PX: f64 = 8.0;

/// エッジ近傍判定に使う、画面端からのマージン（論理px換算）。8px移動しきい値より少し余裕を
/// 持たせた値。誤検知/検知漏れのバランスを見ながら将来調整可能（architecture.md 8.1章）。
const EDGE_ZONE_MARGIN_LOGICAL_PX: f64 = 32.0;

/// フックプロシージャ（`unsafe extern "system" fn`、ユーザーデータポインタを直接渡せない）から
/// アクセスするための共有状態。プロセス内に1インスタンスのみを想定する
/// （このアプリでは`AppState`に1つだけ`DragWatcher`を保持する設計のため問題ない。
/// 呼び出し元への報告事項: 迷った設計判断）。
struct HookState {
    down_pos: Option<POINT>,
    dragging: bool,
    /// このドラッグセッション（押下〜離すまで）で既に`on_start`を呼んだか。
    /// エッジゾーン外にいる間は`dragging`がtrueになっても`on_start`を呼ばないため、
    /// `dragging`とは別に管理する。
    triggered: bool,
    edge: Option<EdgeGeometry>,
    on_start: Box<dyn Fn() + Send + Sync>,
    on_end: Box<dyn Fn() + Send + Sync>,
}

static HOOK_STATE: OnceLock<Mutex<HookState>> = OnceLock::new();

/// フックプロシージャ内で直接コールバックを呼ぶと、Windowsの低レベルフックタイムアウト
/// （既定約300ms、`LowLevelHooksTimeout`）に抵触しフックが強制解除されるおそれがある。
/// そのため、フックプロシージャ内では状態更新とこれらのカスタムスレッドメッセージの投函のみ行い、
/// 実際のコールバック呼び出しはメッセージループ側（`run_message_loop`）で行う。
const WM_APP_DRAG_START: u32 = 0x8000 + 1;
const WM_APP_DRAG_END: u32 = 0x8000 + 2;

pub struct WindowsDragWatcher {
    thread_id: Option<u32>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl WindowsDragWatcher {
    pub fn new() -> Self {
        Self {
            thread_id: None,
            join_handle: None,
        }
    }
}

impl Default for WindowsDragWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DragWatcher for WindowsDragWatcher {
    fn start(
        &mut self,
        on_start: Box<dyn Fn() + Send + Sync>,
        on_end: Box<dyn Fn() + Send + Sync>,
        edge: Option<EdgeGeometry>,
    ) -> Result<(), ShelfError> {
        // HOOK_STATEはOnceLockのため2回目以降のstart()では上書きされない。stop()を挟まず
        // start()を連続で呼ぶ運用は想定していない（drag_watch::set_enabledは必ずstop()してから
        // start()する）。
        let _ = HOOK_STATE.set(Mutex::new(HookState {
            down_pos: None,
            dragging: false,
            triggered: false,
            edge,
            on_start,
            on_end,
        }));

        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        let join_handle = std::thread::Builder::new()
            .name("drag-watch-win".into())
            .spawn(move || run_message_loop(tx))
            .map_err(|e| {
                ShelfError::Internal(format!("ドラッグ監視スレッドの起動に失敗しました: {e}"))
            })?;

        let thread_id = rx.recv().map_err(|_| {
            ShelfError::Internal("ドラッグ監視スレッドの初期化に失敗しました".into())
        })?;
        if thread_id == 0 {
            return Err(ShelfError::Internal(
                "低レベルマウスフックの登録に失敗しました".into(),
            ));
        }

        self.thread_id = Some(thread_id);
        self.join_handle = Some(join_handle);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(thread_id) = self.thread_id.take() {
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// 監視専用スレッドで実行するメッセージループ本体。
/// フック登録・カスタムメッセージのディスパッチ・フック解除までを行う。
fn run_message_loop(tx: std::sync::mpsc::Sender<u32>) {
    unsafe {
        // WH_MOUSE_LLはhMod=NULL・dwThreadId=0（システム全体）で登録する（MSDN仕様）。
        // 低レベルフックは登録したスレッドのコンテキストで呼び出されるため、このスレッドに
        // メッセージループが必須。
        let hook: HHOOK = match SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), None, 0) {
            Ok(hook) => hook,
            Err(_) => {
                // 呼び出し元(start)がrx.recv()でブロックしないよう、失敗時は0を送る
                let _ = tx.send(0);
                return;
            }
        };

        let thread_id = windows::Win32::System::Threading::GetCurrentThreadId();
        let _ = tx.send(thread_id);

        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if !ret.as_bool() {
                break; // WM_QUIT（stop()から送られる）
            }
            if msg.message == WM_APP_DRAG_START {
                invoke_on_start();
            } else if msg.message == WM_APP_DRAG_END {
                invoke_on_end();
            } else {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        let _ = UnhookWindowsHookEx(hook);
    }
}

/// `WH_MOUSE_LL`フックプロシージャ。状態更新のみ行い、実処理はメッセージループへ委譲する。
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wparam, lparam);
    }

    let msg = wparam.0 as u32;
    if msg == WM_LBUTTONDOWN || msg == WM_MOUSEMOVE || msg == WM_LBUTTONUP {
        let info = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        handle_event(msg, info.pt);
    }

    CallNextHookEx(None, code, wparam, lparam)
}

/// マウスイベントに応じて`HookState`を更新し、しきい値を超えた/ボタンが離された時点で
/// メッセージループ宛にカスタムスレッドメッセージを投函する。
fn handle_event(msg: u32, pt: POINT) {
    let Some(state_lock) = HOOK_STATE.get() else {
        return;
    };
    let mut state = state_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // 低レベルフックはフックを登録したスレッドのコンテキストで呼び出されるため、
    // ここでの`GetCurrentThreadId()`は常に監視スレッド自身のIDになる。
    let self_thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };

    if msg == WM_LBUTTONDOWN {
        state.down_pos = Some(pt);
        state.dragging = false;
        state.triggered = false;
    } else if msg == WM_MOUSEMOVE {
        if state.triggered {
            return;
        }
        if !state.dragging {
            let Some(down_pos) = state.down_pos else {
                return;
            };
            let dx = (pt.x - down_pos.x) as f64;
            let dy = (pt.y - down_pos.y) as f64;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance >= threshold_physical_px() {
                state.dragging = true;
            }
        }
        if !state.dragging {
            return;
        }
        // エッジ近傍判定: ジオメトリ取得に失敗している場合（edge: None）は安全側に倒し
        // 従来通り無条件で発火する。
        let in_zone = match &state.edge {
            Some(edge) => in_edge_zone(pt, edge),
            None => true,
        };
        if !in_zone {
            return;
        }
        state.triggered = true;
        drop(state);
        unsafe {
            let _ = PostThreadMessageW(self_thread_id, WM_APP_DRAG_START, WPARAM(0), LPARAM(0));
        }
    } else if msg == WM_LBUTTONUP {
        let was_triggered = state.triggered;
        state.down_pos = None;
        state.dragging = false;
        state.triggered = false;
        drop(state);
        if was_triggered {
            unsafe {
                let _ = PostThreadMessageW(self_thread_id, WM_APP_DRAG_END, WPARAM(0), LPARAM(0));
            }
        }
    }
}

/// カーソル位置（物理px、スクリーン座標）が、設定済みシェルフ表示端の近傍ゾーン内にあるかを判定する。
fn in_edge_zone(pt: POINT, edge: &EdgeGeometry) -> bool {
    let margin = margin_physical_px(edge.scale_factor).round() as i32;
    let min_x = edge.work_area_x;
    let min_y = edge.work_area_y;
    let max_x = edge.work_area_x + edge.work_area_width as i32;
    let max_y = edge.work_area_y + edge.work_area_height as i32;
    match edge.edge {
        crate::settings::ShelfEdge::Left => {
            pt.x >= min_x && pt.x <= min_x + margin && pt.y >= min_y && pt.y <= max_y
        }
        crate::settings::ShelfEdge::Right => {
            pt.x <= max_x && pt.x >= max_x - margin && pt.y >= min_y && pt.y <= max_y
        }
        crate::settings::ShelfEdge::Top => {
            pt.y >= min_y && pt.y <= min_y + margin && pt.x >= min_x && pt.x <= max_x
        }
        crate::settings::ShelfEdge::Bottom => {
            pt.y <= max_y && pt.y >= max_y - margin && pt.x >= min_x && pt.x <= max_x
        }
    }
}

/// エッジゾーンのマージン（論理px）を物理px換算する。`EdgeGeometry.scale_factor`（Tauriの
/// monitor APIから取得した値）を使う。`threshold_physical_px`が使う`GetDpiForSystem`ベースの
/// スケール値と厳密には別経路の値だが、通常はどちらもプライマリディスプレイのDPIから算出される
/// ため実用上の差異は無い想定（呼び出し元への報告事項: 迷った設計判断。ズレが気になる場合は
/// 将来どちらかに統一する余地がある）。
fn margin_physical_px(scale_factor: f64) -> f64 {
    EDGE_ZONE_MARGIN_LOGICAL_PX * scale_factor
}

/// 指定した画面座標（マウス押下位置）がウィンドウのタイトルバー（キャプション領域）かどうかを
/// `WM_NCHITTEST`で判定する。ウィンドウのドラッグ移動はほぼ必ずタイトルバーの掴みから始まるため、
/// これを弾くことでウィンドウ移動による誤発火を減らす（ユーザー報告「ウィンドウを画面端に持って
/// いくとまだシェルフが出る」への対策、macOS版の`NSPasteboard(name: .drag)`判定に相当する
/// Windows版の代替策。ファイル冒頭「タイトルバードラッグの除外について」参照）。
///
/// **重要**: この関数は`SendMessageW`という同期・ブロッキング呼び出しを含むため、
/// `handle_event`/`hook_proc`（`WH_MOUSE_LL`フックプロシージャの呼び出し系列）の中では
/// 絶対に呼び出してはならない（ファイル冒頭コメント「低レベルフックタイムアウト」参照）。
/// 呼び出しは`invoke_on_start`（メッセージループスレッドの通常のディスパッチ処理内、
/// フックプロシージャの外）からのみ行うこと。
fn is_title_bar_hit(pt: POINT) -> bool {
    unsafe {
        let hwnd = WindowFromPoint(pt);
        if hwnd.is_invalid() {
            return false;
        }
        // WM_NCHITTESTのlParamは、x/yそれぞれ16bit値としてパックしたスクリーン座標
        // （Win32の伝統的なMAKELPARAMマクロと同じ形）。
        let lparam = LPARAM((((pt.y as u32) << 16) | (pt.x as u32 & 0xFFFF)) as isize);
        let result: LRESULT = SendMessageW(hwnd, WM_NCHITTEST, Some(WPARAM(0)), Some(lparam));
        result.0 == HTCAPTION as isize
    }
}

fn invoke_on_start() {
    let Some(state) = HOOK_STATE.get() else {
        return;
    };
    let down_pos = {
        let guard = state.lock().unwrap_or_else(|p| p.into_inner());
        guard.down_pos
    };
    // SendMessageW（ブロッキング呼び出し）はロックを保持したまま行わない。
    if let Some(pos) = down_pos {
        if is_title_bar_hit(pos) {
            // ウィンドウ移動（タイトルバードラッグ）とみなし、on_startを呼ばない。
            return;
        }
    }
    let guard = state.lock().unwrap_or_else(|p| p.into_inner());
    (guard.on_start)();
}

fn invoke_on_end() {
    if let Some(state) = HOOK_STATE.get() {
        let guard = state.lock().unwrap_or_else(|p| p.into_inner());
        (guard.on_end)();
    }
}

/// 論理px換算のしきい値を、システムDPIから物理px換算する（architecture.md 8.1章:
/// 「DPI非依存の論理px換算」）。マルチモニタでモニタごとにDPIが異なるケースの精密な追従は
/// スコープ外とし、システム全体のDPI（`GetDpiForSystem`）で近似する簡略化とした。
fn threshold_physical_px() -> f64 {
    let dpi = unsafe { GetDpiForSystem() };
    let scale = dpi as f64 / 96.0;
    DRAG_THRESHOLD_LOGICAL_PX * scale
}
