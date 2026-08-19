//! Windows向け「ドラッグ操作開始」ヒューリスティック検知実装（F-08, architecture.md 8.1章）。
//!
//! `WH_MOUSE_LL`（低レベルマウスフック）で「左ボタン押下→一定距離（DPI換算で既定8論理px相当）
//! 以上の移動」というパターンを検知し、ドラッグ操作の可能性が高いと判定してコールバックを呼ぶ。
//! OLEドラッグそのものは検知できない（Windowsにはグローバルなドラッグ開始通知APIが存在しない）
//! ため、あくまでヒューリスティックであり、ファイルを伴わない単なるドラッグ選択やウィンドウ移動
//! でも反応しうる。誤検知の実害は小さい（シェルフが表示されるだけ）という判断のもと許容する
//! （architecture.md 8.1章）。
//!
//! `#[cfg(target_os = "windows")]`配下のためこのLinux開発コンテナではコンパイル対象外であり、
//! **実機（Windows）での動作確認は未実施**。構文的な妥当性とロジックの妥当性のレビューに留まる
//! （architecture.md 7章）。
//!
//! 簡略化した点（呼び出し元への報告事項）: architecture.md 8.1章は「一定時間（既定800ms）操作が
//! 無ければ自動的に表示状態を解除して**良い**」と許容的に書いており必須要件ではないため、
//! タイマーによる無操作タイムアウト解除は実装せず、「ボタンを離す」ことのみを終了条件とする
//! 簡易実装とした。

use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HHOOK, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_QUIT,
};

use crate::error::ShelfError;

use super::DragWatcher;

/// ドラッグ開始とみなす移動距離（論理px換算、architecture.md 8.1章の既定値）。
const DRAG_THRESHOLD_LOGICAL_PX: f64 = 8.0;

/// フックプロシージャ（`unsafe extern "system" fn`、ユーザーデータポインタを直接渡せない）から
/// アクセスするための共有状態。プロセス内に1インスタンスのみを想定する
/// （このアプリでは`AppState`に1つだけ`DragWatcher`を保持する設計のため問題ない。
/// 呼び出し元への報告事項: 迷った設計判断）。
struct HookState {
    down_pos: Option<POINT>,
    dragging: bool,
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
    ) -> Result<(), ShelfError> {
        // HOOK_STATEはOnceLockのため2回目以降のstart()では上書きされない。stop()を挟まず
        // start()を連続で呼ぶ運用は想定していない（drag_watch::set_enabledは必ずstop()してから
        // start()する）。
        let _ = HOOK_STATE.set(Mutex::new(HookState {
            down_pos: None,
            dragging: false,
            on_start,
            on_end,
        }));

        let (tx, rx) = std::sync::mpsc::channel::<u32>();
        let join_handle = std::thread::Builder::new()
            .name("drag-watch-win".into())
            .spawn(move || run_message_loop(tx))
            .map_err(|e| ShelfError::Internal(format!("ドラッグ監視スレッドの起動に失敗しました: {e}")))?;

        let thread_id = rx
            .recv()
            .map_err(|_| ShelfError::Internal("ドラッグ監視スレッドの初期化に失敗しました".into()))?;
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
    } else if msg == WM_MOUSEMOVE {
        if state.dragging {
            return;
        }
        let Some(down_pos) = state.down_pos else {
            return;
        };
        let dx = (pt.x - down_pos.x) as f64;
        let dy = (pt.y - down_pos.y) as f64;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance >= threshold_physical_px() {
            state.dragging = true;
            drop(state);
            unsafe {
                let _ = PostThreadMessageW(self_thread_id, WM_APP_DRAG_START, WPARAM(0), LPARAM(0));
            }
        }
    } else if msg == WM_LBUTTONUP {
        let was_dragging = state.dragging;
        state.down_pos = None;
        state.dragging = false;
        drop(state);
        if was_dragging {
            unsafe {
                let _ = PostThreadMessageW(self_thread_id, WM_APP_DRAG_END, WPARAM(0), LPARAM(0));
            }
        }
    }
}

fn invoke_on_start() {
    if let Some(state) = HOOK_STATE.get() {
        let guard = state.lock().unwrap_or_else(|p| p.into_inner());
        (guard.on_start)();
    }
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
