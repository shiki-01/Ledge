//! システムトレイ常駐（F-18）。
//!
//! 左クリックでシェルフ表示切り替え、右クリックメニューは
//! 「シェルフを表示」「設定...」「終了」。「設定...」はPhase3でSettings.svelte（設定タブ）を
//! 実装したため有効化した。

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use crate::error::ShelfError;
use crate::window;

const MENU_ID_SHOW_SHELF: &str = "show_shelf";
const MENU_ID_SETTINGS: &str = "settings";
const MENU_ID_QUIT: &str = "quit";

/// トレイアイコンとメニューを構築する。
pub fn setup_tray(app: &AppHandle) -> Result<(), ShelfError> {
    let show_item = MenuItem::with_id(
        app,
        MENU_ID_SHOW_SHELF,
        "シェルフを表示",
        true,
        None::<&str>,
    )
    .map_err(tray_err)?;
    let settings_item = MenuItem::with_id(app, MENU_ID_SETTINGS, "設定...", true, None::<&str>)
        .map_err(tray_err)?;
    let quit_item =
        MenuItem::with_id(app, MENU_ID_QUIT, "終了", true, None::<&str>).map_err(tray_err)?;

    let menu =
        Menu::with_items(app, &[&show_item, &settings_item, &quit_item]).map_err(tray_err)?;

    let icon = app.default_window_icon().cloned().ok_or_else(|| {
        ShelfError::Internal("トレイアイコン用の既定アイコンが見つかりません".into())
    })?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        // 左クリックでのメニュー表示は行わず、独自にシェルフのトグル動作を割り当てる
        .show_menu_on_left_click(false)
        .tooltip("Ledge")
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_ID_SHOW_SHELF => window::show_shelf(app),
            MENU_ID_SETTINGS => window::show_settings(app),
            MENU_ID_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::toggle_shelf(tray.app_handle());
            }
        })
        .build(app)
        .map_err(tray_err)?;

    Ok(())
}

fn tray_err<E: std::fmt::Display>(e: E) -> ShelfError {
    ShelfError::Internal(format!("トレイアイコンの初期化に失敗しました: {e}"))
}
