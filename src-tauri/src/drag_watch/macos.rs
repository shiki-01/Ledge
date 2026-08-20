//! macOS向け「ドラッグ操作開始」ヒューリスティック検知実装（F-08, architecture.md 10.1章）。
//!
//! `NSEvent.addGlobalMonitorForEvents(matching:handler:)`でシステム全体の`.leftMouseDown` /
//! `.leftMouseDragged` / `.leftMouseUp`を監視し、Windows版（`drag_watch/windows.rs`の
//! `WH_MOUSE_LL`ヒューリスティック）と同じロジック（左ボタン押下位置から一定距離以上の移動が
//! 継続したら「ドラッグ操作の可能性が高い」と判定する。ボタンを離したら終了とみなす）を踏襲する
//! （architecture.md 10.1章）。private APIには依存しない。
//!
//! Windows版と異なり、`NSEvent`の`locationInWindow`はmacOSの解像度非依存座標系（ポイント単位）
//! で渡されるため、`windows.rs`の`GetDpiForSystem`に相当するDPIスケール換算は不要
//! （macOSのAPI自体が既に論理px相当の値を返す設計のため）。
//!
//! ## 権限要件（architecture.md 10.1章）
//! グローバルモニタは自プロセスにフォーカスが無い他アプリのイベントも監視するため、
//! 「アクセシビリティ」権限（`System Settings > Privacy & Security > Accessibility`）の許可が
//! 必要（macOS本体のAPI仕様上の制約であり、Private API依存ではなく公開APIの正規の権限モデル）。
//! 権限が無い状態では`addGlobalMonitorForEventsMatchingMask_handler`自体はエラーにならず、
//! ハンドラが単に一度も呼ばれない。起動時に`accessibility_trusted()`で権限有無を確認できるよう
//! 用意している（実際に設定画面へ誘導するUI導線を組むかは呼び出し元の判断に委ねる。
//! architecture.md 10.1章「初回起動時のUX」参照）。
//!
//! ## メインスレッドアフィニティについて（迷った設計判断、呼び出し元への報告事項）
//! AppKitのグローバルイベントモニタ登録/解除はメインスレッド（`NSApplication`のrun loopが
//! 動いているスレッド）から行う必要があるが、`DragWatcher::start`/`stop`はTauriの設定変更
//! コマンド（`commands/settings.rs`の`update_settings`）からも呼ばれ、そのコマンドハンドラが
//! 必ずメインスレッドで実行される保証は無い。`DragWatcher` traitにAppHandle等を渡す設計変更も
//! 検討したが、Windows版とのtrait共通化（`drag_watch/mod.rs`）を崩したくなかったため、
//! `dispatch2::run_on_main`（呼び出し元がメインスレッドならそのまま実行、そうでなければ
//! メインスレッドへ同期ディスパッチする）を使い、trait自体には手を入れずに解決した。
//! 登録済みモニタのトークン（`Retained<AnyObject>`、Objective-Cオブジェクトのため本来`Send`では
//! ない）は`dispatch2::MainThreadBound`で包み、実際のアクセス・解放は必ずメインスレッド上で
//! 行われることを型レベルで保証している。
//!
//! ## この実装の検証状況
//! `#[cfg(target_os = "macos")]`配下のためこのLinux開発コンテナではコンパイル対象外であり、
//! **実機（macOS）でのビルド・動作確認は未実施**。ただし`objc2`/`objc2-app-kit`/`block2`/
//! `dispatch2`各クレートの実ソース（`static.crates.io`から`Cargo.lock`記載バージョンを直接取得し
//! 展開したもの）を参照し、ここで使用しているメソッドシグネチャ（`addGlobalMonitorForEventsMatchingMask_handler`
//! `removeMonitor`, `NSEvent::r#type`, `NSEvent::locationInWindow`, `dispatch2::run_on_main`,
//! `dispatch2::MainThreadBound`等）が実際に存在し型が一致することは確認済み。とはいえ実機ビルドでの
//! 最終確認は必要（windows.rsと同じ扱い、architecture.md 7章）。
//!
//! ## エッジ近傍判定の追加について（座標変換、実機未検証のリスクが最も高い箇所）
//! Windows版と同じ理由（画面上のどこでクリック&ドラッグしても反応してしまい邪魔、という
//! ユーザー報告）で、設定済みのシェルフ表示端（`AppSettings.shelf_edge`）に近いエリアに入った時
//! だけ発火するよう変更した。`NSEvent.locationInWindow()`はグローバルモニタ（windowがnil）の
//! イベントでは、AppKitネイティブの座標系（**左下原点・y上向き・ポイント単位**）でスクリーン座標を
//! 返す一方、`EdgeGeometry`はTauri/winit流（**左上原点・y下向き・物理px**）であり、両者の座標系が
//! 異なる。プライマリディスプレイ（メニューバーがある画面、`NSScreen.screens()`の先頭要素、原点は
//! 必ず`(0,0)`）の高さ（ポイント単位）を使ってy軸を反転して変換している（`to_top_left_points`）。
//! この座標変換ロジックは実機で未検証であり、他の箇所以上に実機（macOS）での動作確認が必要
//! （windows.rsやこのファイル冒頭の「検証状況」と同じ扱い、architecture.md 7章）。

use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use block2::RcBlock;
use dispatch2::{run_on_main, MainThreadBound};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventType, NSScreen};
use objc2_foundation::NSPoint;

use crate::error::ShelfError;
use crate::settings::ShelfEdge;

use super::{DragWatcher, EdgeGeometry};

/// ドラッグ開始とみなす移動距離（ポイント単位。Windows版`DRAG_THRESHOLD_LOGICAL_PX`と同じ既定値、
/// architecture.md 10.1章・8.1章）。
const DRAG_THRESHOLD_POINTS: f64 = 8.0;

/// エッジ近傍判定に使う、画面端からのマージン（ポイント単位）。Windows版
/// `EDGE_ZONE_MARGIN_LOGICAL_PX`と同じ既定値。マージンはポイント単位でそのまま使うため
/// DPI換算は不要（ファイル冒頭コメント参照）。
const EDGE_ZONE_MARGIN_POINTS: f64 = 32.0;

/// イベントハンドラのクロージャ内からアクセスするための共有状態
/// （windows.rsの`HookState`と同じ設計だが、こちらはインスタンスごとの`Arc<Mutex<_>>`で持つ。
/// Windows実装は`OnceLock`によるプロセス内単一状態だったが、mac実装はNSEventのブロックが
/// 直接この状態をキャプチャできるためグローバル状態は不要）。
struct WatcherState {
    down_pos: Option<NSPoint>,
    dragging: bool,
    /// このドラッグセッション（押下〜離すまで）で既に`on_start`を呼んだか（windows.rsの
    /// `HookState::triggered`と同じ役割）。
    triggered: bool,
    edge: Option<EdgeGeometry>,
    on_start: Box<dyn Fn() + Send + Sync>,
    on_end: Box<dyn Fn() + Send + Sync>,
}

pub struct MacDragWatcher {
    /// 登録済みグローバルモニタのトークン。`MainThreadBound`で包むことで、実体（Objective-C
    /// オブジェクト）へのアクセス・解放が必ずメインスレッドで行われることを型レベルで保証する。
    monitor: Option<MainThreadBound<Retained<AnyObject>>>,
}

impl MacDragWatcher {
    pub fn new() -> Self {
        Self { monitor: None }
    }
}

impl Default for MacDragWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl DragWatcher for MacDragWatcher {
    fn start(
        &mut self,
        on_start: Box<dyn Fn() + Send + Sync>,
        on_end: Box<dyn Fn() + Send + Sync>,
        edge: Option<EdgeGeometry>,
    ) -> Result<(), ShelfError> {
        let state = Arc::new(Mutex::new(WatcherState {
            down_pos: None,
            dragging: false,
            triggered: false,
            edge,
            on_start,
            on_end,
        }));

        // メインスレッドで登録する（モジュール冒頭コメント「メインスレッドアフィニティについて」参照）。
        let registered = run_on_main(move |mtm| register_monitor(mtm, state));

        let monitor = registered.ok_or_else(|| {
            ShelfError::Internal("ドラッグ監視用グローバルイベントモニタの登録に失敗しました".into())
        })?;
        self.monitor = Some(monitor);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            run_on_main(move |mtm| {
                let object = monitor.into_inner(mtm);
                // SAFETY: `object`は`register_monitor`で`addGlobalMonitorForEventsMatchingMask_handler`
                // が返したモニタトークンそのものであり、`removeMonitor:`に渡すのに適切な型である。
                unsafe { NSEvent::removeMonitor(&object) };
            });
        }
    }
}

/// メインスレッド上でグローバルイベントモニタを登録する（`run_on_main`のクロージャから呼ばれる）。
fn register_monitor(
    mtm: MainThreadMarker,
    state: Arc<Mutex<WatcherState>>,
) -> Option<MainThreadBound<Retained<AnyObject>>> {
    let mask = NSEventMask::LeftMouseDown | NSEventMask::LeftMouseDragged | NSEventMask::LeftMouseUp;

    let block = RcBlock::new(move |event: NonNull<NSEvent>| {
        // SAFETY: AppKitがハンドラ呼び出し時に渡す`NSEvent`は常に有効なポインタである。
        let event = unsafe { event.as_ref() };
        handle_event(&state, event);
    });

    let monitor = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &block);
    monitor.map(|m| MainThreadBound::new(m, mtm))
}

/// マウスイベントに応じて状態を更新し、しきい値を超えた/ボタンが離された時点でコールバックを呼ぶ
/// （windows.rsの`handle_event`と同じロジック、architecture.md 10.1章）。エッジ近傍判定
/// （ファイル冒頭コメント参照）を追加したため、しきい値を超えた後さらにゾーン内かどうかを見てから
/// `on_start`を呼ぶ。
///
/// NSEventのグローバルモニタのハンドラはメインスレッド（run loopを回しているスレッド）上で
/// 直接呼ばれるため、Windows版（`WH_MOUSE_LL`のタイムアウト制約を避けるためカスタムスレッド
/// メッセージ経由でメインスレッドへ処理を渡していた）と異なり、コールバックをここで直接呼んで
/// 問題ない。同じ理由で、ゾーン判定に必要な`MainThreadMarker`もこの関数内で`MainThreadMarker::new()`
/// により直接取得できる（`register_monitor`から`mtm`を明示的に受け渡す設計も検討したが、
/// 呼び出し元がメインスレッドであることは既に保証されているためこちらを選んだ。
/// 呼び出し元への報告事項: 迷った設計判断）。
fn handle_event(state: &Mutex<WatcherState>, event: &NSEvent) {
    let event_type = unsafe { event.r#type() };
    let loc = unsafe { event.locationInWindow() };

    let mut guard = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    if event_type == NSEventType::LeftMouseDown {
        guard.down_pos = Some(loc);
        guard.dragging = false;
        guard.triggered = false;
    } else if event_type == NSEventType::LeftMouseDragged {
        if guard.triggered {
            return;
        }
        if !guard.dragging {
            let Some(down_pos) = guard.down_pos else {
                return;
            };
            let dx = loc.x - down_pos.x;
            let dy = loc.y - down_pos.y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance >= DRAG_THRESHOLD_POINTS {
                guard.dragging = true;
            }
        }
        if !guard.dragging {
            return;
        }
        // エッジ近傍判定: ジオメトリ取得に失敗している場合（edge: None）、あるいはメインスレッド
        // マーカー/スクリーン情報が取得できなかった場合は安全側に倒し従来通り無条件で発火する。
        let in_zone = match &guard.edge {
            Some(edge) => MainThreadMarker::new()
                .and_then(|mtm| to_top_left_points(loc, mtm))
                .map(|pt| in_edge_zone(pt, edge))
                .unwrap_or(true),
            None => true,
        };
        if !in_zone {
            return;
        }
        guard.triggered = true;
        (guard.on_start)();
    } else if event_type == NSEventType::LeftMouseUp {
        let was_triggered = guard.triggered;
        guard.down_pos = None;
        guard.dragging = false;
        guard.triggered = false;
        if was_triggered {
            (guard.on_end)();
        }
    }
}

/// AppKitのグローバル座標（左下原点・y上向き・ポイント単位）を、Tauri/winit流の座標系
/// （左上原点・y下向き）へ変換する（ポイント単位のまま。物理pxへの換算は`in_edge_zone`側で行う）。
/// プライマリディスプレイ（`NSScreen.screens()`の先頭要素、原点は必ず`(0,0)`）の高さを使って
/// y軸を反転する。
fn to_top_left_points(loc: NSPoint, mtm: MainThreadMarker) -> Option<NSPoint> {
    let screens = NSScreen::screens(mtm);
    let primary = screens.firstObject()?;
    let height = primary.frame().size.height;
    Some(NSPoint {
        x: loc.x,
        y: height - loc.y,
    })
}

/// カーソル位置（左上原点・y下向き・ポイント単位）が、設定済みシェルフ表示端の近傍ゾーン内に
/// あるかを判定する（windows.rsの`in_edge_zone`と同じ考え方）。`EdgeGeometry`は物理px・
/// `scale_factor`付きのため、比較の前にポイント単位へ変換する。
fn in_edge_zone(pt: NSPoint, edge: &EdgeGeometry) -> bool {
    let min_x = edge.work_area_x as f64 / edge.scale_factor;
    let min_y = edge.work_area_y as f64 / edge.scale_factor;
    let max_x = min_x + edge.work_area_width as f64 / edge.scale_factor;
    let max_y = min_y + edge.work_area_height as f64 / edge.scale_factor;
    let margin = EDGE_ZONE_MARGIN_POINTS;
    match edge.edge {
        ShelfEdge::Left => pt.x >= min_x && pt.x <= min_x + margin && pt.y >= min_y && pt.y <= max_y,
        ShelfEdge::Right => pt.x <= max_x && pt.x >= max_x - margin && pt.y >= min_y && pt.y <= max_y,
        ShelfEdge::Top => pt.y >= min_y && pt.y <= min_y + margin && pt.x >= min_x && pt.x <= max_x,
        ShelfEdge::Bottom => pt.y <= max_y && pt.y >= max_y - margin && pt.x >= min_x && pt.x <= max_x,
    }
}

/// Accessibility権限（`System Settings > Privacy & Security > Accessibility`）が許可されているかを
/// 確認する。`false`の場合、`NSEvent`のグローバルモニタはエラーにならず単にハンドラが呼ばれない
/// ため、起動時にこの関数で確認しユーザーへ案内する用途を想定している（architecture.md 10.1章
/// 「初回起動時のUX」）。ここでは確認関数の提供までをスコープとし、案内UIの実装は呼び出し元の
/// 判断に委ねる（タスク指示: 過剰実装をしない）。
pub fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

// AXIsProcessTrusted()はApplicationServices(HIServices)フレームワークのC関数であり、
// objc2/objc2-app-kitのバインディング対象外（AppKitではなくApplicationServicesの一部）のため、
// 直接externブロックで宣言する。
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}
