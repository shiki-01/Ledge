//! シェルフウィンドウの表示制御・画面端への配置ロジック（architecture.md 5章）。
//!
//! ディレクトリ構成としてはarchitecture.md 1章に明記が無いが、トレイ（tray.rs）と
//! グローバルショートカット（shortcut/mod.rs）の両方から「シェルフの表示/非表示切り替え」
//! というロジックを共有する必要があるため、小さな共通モジュールとして切り出した
//! （呼び出し元への報告事項: 迷った設計判断）。

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};
use tracing::{error, warn};

use crate::settings::{AppSettings, ShelfEdge};

/// tauri.conf.jsonで定義したシェルフウィンドウのlabel。
pub const MAIN_WINDOW_LABEL: &str = "main";

/// シェルフの幅（論理ピクセル）。Phase1では固定値とし、可変化はPhase3の表示設定で対応する。
const SHELF_WIDTH_LOGICAL: f64 = 300.0;

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

fn show(app: &AppHandle, window: &WebviewWindow) {
    reposition(window, app);
    if let Err(e) = window.show() {
        error!(error = %e, "シェルフウィンドウの表示に失敗しました");
    }
    if let Err(e) = window.set_focus() {
        warn!(error = %e, "シェルフウィンドウへのフォーカス設定に失敗しました");
    }
}

fn hide(window: &WebviewWindow) {
    if let Err(e) = window.hide() {
        error!(error = %e, "シェルフウィンドウの非表示に失敗しました");
    }
}

/// 設定に基づいてシェルフウィンドウを画面端へ配置する。
///
/// Phase1では「起動時/表示時にプライマリディスプレイの作業領域へ合わせて再配置する」ところまでを
/// 実装し、マルチモニタ選択や上下端（top/bottom）の完全な作り込みはPhase3で対応する
/// （architecture.md 5章: 「動的再配置の完全な作り込みはPhase3でよいが、起動時に右端へ配置する
/// 処理は入れること」という指示に基づく判断）。top/bottomが指定された場合は現時点ではrightと
/// 同じ配置にフォールバックする。
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
    let width_physical = ((SHELF_WIDTH_LOGICAL * scale).round() as i32).max(1);
    let height_physical = work_area.size.height as i32;

    let x = match settings.shelf_edge {
        ShelfEdge::Left => work_area.position.x,
        ShelfEdge::Right | ShelfEdge::Top | ShelfEdge::Bottom => {
            if !matches!(settings.shelf_edge, ShelfEdge::Right) {
                warn!(
                    edge = ?settings.shelf_edge,
                    "top/bottom方向の配置はPhase3で実装予定のため、rightと同じ配置にフォールバックします"
                );
            }
            work_area.position.x + work_area.size.width as i32 - width_physical
        }
    };
    let y = work_area.position.y;

    if let Err(e) = window.set_size(PhysicalSize::new(width_physical as u32, height_physical.max(1) as u32)) {
        warn!(error = %e, "シェルフウィンドウのサイズ設定に失敗しました");
    }
    if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
        warn!(error = %e, "シェルフウィンドウの位置設定に失敗しました");
    }
}
