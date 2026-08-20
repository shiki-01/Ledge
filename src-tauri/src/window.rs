//! シェルフウィンドウの表示制御・画面端への配置ロジック（architecture.md 5章）。
//!
//! ディレクトリ構成としてはarchitecture.md 1章に明記が無いが、トレイ（tray.rs）と
//! グローバルショートカット（shortcut/mod.rs）の両方から「シェルフの表示/非表示切り替え」
//! というロジックを共有する必要があるため、小さな共通モジュールとして切り出した
//! （呼び出し元への報告事項: 迷った設計判断）。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};
use tracing::{error, warn};

use crate::settings::{AppSettings, ShelfEdge};

/// tauri.conf.jsonで定義したシェルフウィンドウのlabel。
pub const MAIN_WINDOW_LABEL: &str = "main";

/// シェルフの厚み（論理ピクセル）。表示端が左右なら幅、上下なら高さに使う
/// （Phase3で可変化する場合は`AppSettings`に厚み用フィールドを追加する余地を残す。
/// 今回のスコープでは固定値のまま、位置（`ShelfEdge`）のみ可変にする）。
const SHELF_THICKNESS_LOGICAL: f64 = 300.0;

/// フロントエンドがトレイの「設定...」から開かれたことを検知するためのイベント名。
const EVENT_OPEN_SETTINGS: &str = "shelf://open-settings";

/// ドラッグ開始検知（F-08）による自動表示中かどうかを示すフラグ。
/// ユーザーが明示的に開いた状態（ホットキー/トレイ操作）と区別するために使う
/// （architecture.md 8.1章: 「ホットキーで開いた状態と自動表示状態を区別するフラグを持つ」）。
static AUTO_SHOWN: AtomicBool = AtomicBool::new(false);

/// シェルフウィンドウの表示/非表示を切り替える（F-01）。
pub fn toggle_shelf(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        error!("シェルフウィンドウが見つかりません");
        return;
    };

    match window.is_visible() {
        Ok(true) => hide(&window),
        _ => show(app, &window),
    }
}

/// シェルフウィンドウを表示する（トレイの「シェルフを表示」メニュー等から呼ばれる）。
pub fn show_shelf(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        error!("シェルフウィンドウが見つかりません");
        return;
    };
    show(app, &window);
}

/// シェルフウィンドウを表示し、フロントエンドへ「設定タブを開け」と通知する
/// （トレイの「設定...」メニューから呼ばれる。F-19/F-10等の設定UI導線）。
pub fn show_settings(app: &AppHandle) {
    show_shelf(app);
    let _ = app.emit(EVENT_OPEN_SETTINGS, ());
}

/// ドラッグ開始検知（F-08）によりシェルフを自動表示する。
/// 既に（ユーザー操作等により）表示済みの場合は何もしない
/// （明示的な表示状態を自動表示扱いで上書きしないため）。
pub fn show_shelf_auto(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        error!("シェルフウィンドウが見つかりません");
        return;
    };
    if matches!(window.is_visible(), Ok(true)) {
        return;
    }
    AUTO_SHOWN.store(true, Ordering::SeqCst);
    show(app, &window);
}

/// ドラッグ終了検知（F-08）により、自動表示中であれば非表示に戻す。
/// ユーザーが明示的に表示した場合（`AUTO_SHOWN=false`）は対象外にする。
pub fn hide_shelf_if_auto(app: &AppHandle) {
    if !AUTO_SHOWN.swap(false, Ordering::SeqCst) {
        return;
    }
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    hide(&window);
}

fn show(app: &AppHandle, window: &WebviewWindow) {
    reposition(window, app);
    if let Err(e) = window.show() {
        error!(error = %e, "シェルフウィンドウの表示に失敗しました");
    }
    if let Err(e) = window.set_focus() {
        warn!(error = %e, "シェルフウィンドウへのフォーカス設定に失敗しました");
    }
    notify_clipboard_panel_visibility(true);
}

fn hide(window: &WebviewWindow) {
    // どちらの経路でシェルフが隠れても「自動表示中」の状態は終了したものとして扱う。
    AUTO_SHOWN.store(false, Ordering::SeqCst);
    if let Err(e) = window.hide() {
        error!(error = %e, "シェルフウィンドウの非表示に失敗しました");
    }
    notify_clipboard_panel_visibility(false);
}

/// メインウィンドウ（Phase2ではシェルフ/クリップボード履歴を1ウィンドウ内タブで切り替える）の
/// 表示状態をmacOSのクリップボード監視ポーリング間隔調整へ伝える（architecture.md 4.1章）。
/// Windows/Linux開発環境ではポーリング間隔の概念自体が無いため何もしない。
#[cfg(target_os = "macos")]
fn notify_clipboard_panel_visibility(visible: bool) {
    crate::clipboard::macos::set_panel_visible(visible);
}

#[cfg(not(target_os = "macos"))]
fn notify_clipboard_panel_visibility(_visible: bool) {}

/// 設定（`AppSettings.shelf_edge`）に基づいてシェルフウィンドウを画面端へ配置する（F-10）。
/// `update_settings`コマンド実行時にも呼ばれ、位置の変更を即座に反映する
/// （architecture.md 8.2章: 「設定変更は即座にウィンドウへ反映する」）。
///
/// マルチモニタでの表示先モニタ選択はスコープ外とし、プライマリディスプレイ固定のまま
/// （architecture.md 8.2章）。
pub fn reposition(window: &WebviewWindow, app: &AppHandle) {
    let settings = match crate::settings::load_settings(app) {
        Ok(settings) => settings,
        Err(e) => {
            warn!(error = %e, "設定の読み込みに失敗したため既定値(right)でシェルフを配置します");
            AppSettings::default()
        }
    };

    let monitor = match window.primary_monitor() {
        Ok(Some(monitor)) => monitor,
        Ok(None) => {
            warn!("プライマリディスプレイの情報が取得できなかったため配置をスキップします");
            return;
        }
        Err(e) => {
            warn!(error = %e, "プライマリディスプレイの情報取得に失敗したため配置をスキップします");
            return;
        }
    };

    let scale = monitor.scale_factor();
    let work_area = *monitor.work_area();
    let thickness_physical = ((SHELF_THICKNESS_LOGICAL * scale).round() as i32).max(1);

    // left/rightは縦長（幅=厚み・高さ=作業領域いっぱい）、top/bottomは横長
    // （幅=作業領域いっぱい・高さ=厚み）で配置する。
    let (width_physical, height_physical, x, y) = match settings.shelf_edge {
        ShelfEdge::Left => (
            thickness_physical,
            work_area.size.height as i32,
            work_area.position.x,
            work_area.position.y,
        ),
        ShelfEdge::Right => (
            thickness_physical,
            work_area.size.height as i32,
            work_area.position.x + work_area.size.width as i32 - thickness_physical,
            work_area.position.y,
        ),
        ShelfEdge::Top => (
            work_area.size.width as i32,
            thickness_physical,
            work_area.position.x,
            work_area.position.y,
        ),
        ShelfEdge::Bottom => (
            work_area.size.width as i32,
            thickness_physical,
            work_area.position.x,
            work_area.position.y + work_area.size.height as i32 - thickness_physical,
        ),
    };

    if let Err(e) = window.set_size(PhysicalSize::new(
        width_physical.max(1) as u32,
        height_physical.max(1) as u32,
    )) {
        warn!(error = %e, "シェルフウィンドウのサイズ設定に失敗しました");
    }
    if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
        warn!(error = %e, "シェルフウィンドウの位置設定に失敗しました");
    }
}
