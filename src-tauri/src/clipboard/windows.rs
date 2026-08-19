//! Windows向けクリップボード監視実装（architecture.md 4.1章）。
//!
//! `AddClipboardFormatListener`によるイベント駆動: メッセージ専用ウィンドウ（`HWND_MESSAGE`）を
//! 1つ作成し、専用スレッドのメッセージループで`WM_CLIPBOARDUPDATE`を受信する。
//!
//! `#[cfg(target_os = "windows")]`配下のためこのLinux開発コンテナではコンパイル対象外であり、
//! 実機（Windows）での静的レビュー・実装意図の確認にとどまる（architecture.md 7章）。
//! windows-rsのモジュールパス（特にCF_*定数の所属モジュール）はcrateのバージョンにより
//! 変わることがあるため、実機ビルド時に`cargo check`で調整が必要になる可能性がある。

use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Arc;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable,
    OpenClipboard, RegisterClipboardFormatW, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::Shell::DragQueryFileW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostThreadMessageW, RegisterClassExW, TranslateMessage, CW_USEDEFAULT, HWND_MESSAGE, MSG,
    WM_CLIPBOARDUPDATE, WM_DESTROY, WM_QUIT, WNDCLASSEXW, WS_OVERLAPPED,
};

use crate::error::ShelfError;

use super::{ClipboardSnapshot, ClipboardWatcher};

/// requirements.md 10.3章: パスワードマネージャー等が付与する除外フォーマット名。
const EXCLUDE_FORMAT_NAME: &str = "ExcludeClipboardContentFromMonitorProcessing";

pub struct WindowsClipboardWatcher {
    thread_id: Option<u32>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl WindowsClipboardWatcher {
    pub fn new() -> Self {
        Self {
            thread_id: None,
            join_handle: None,
        }
    }
}

impl Default for WindowsClipboardWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardWatcher for WindowsClipboardWatcher {
    fn start(
        &mut self,
        on_change: Box<dyn Fn(ClipboardSnapshot) + Send + Sync>,
    ) -> Result<(), ShelfError> {
        let on_change = Arc::new(on_change);
        let (tx, rx) = mpsc::channel::<u32>();

        let join_handle = std::thread::Builder::new()
            .name("clipboard-watcher-win".into())
            .spawn(move || run_message_loop(on_change, tx))
            .map_err(|e| ShelfError::Internal(format!("クリップボード監視スレッドの起動に失敗しました: {e}")))?;

        // ウィンドウ作成が完了し、スレッドIDが送られてくるまで待つ（stop()でのPostThreadMessage用）
        let thread_id = rx
            .recv()
            .map_err(|_| ShelfError::Internal("クリップボード監視スレッドの初期化に失敗しました".into()))?;

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
fn run_message_loop(on_change: Arc<Box<dyn Fn(ClipboardSnapshot) + Send + Sync>>, tx: mpsc::Sender<u32>) {
    unsafe {
        let class_name = to_wide("ShelfDropClipboardWatcher");
        let instance = GetModuleHandleW(None).unwrap_or_default();

        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wnd_class);

        // HWND_MESSAGEを親にすることで、画面に表示されないメッセージ専用ウィンドウを作る
        let hwnd = match CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WS_OVERLAPPED,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND_MESSAGE,
            None,
            Some(instance.into()),
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(_) => {
                // 呼び出し元(start)がrx.recv()でブロックしないよう、失敗時もスレッドIDを流す
                let _ = tx.send(0);
                return;
            }
        };

        // ここでは`GetCurrentThreadId`をラップした`windows`クレートのヘルパーは使わず、
        // `GetWindowThreadProcessId`経由ではなく`GetCurrentThreadId`を直接呼ぶ想定
        let thread_id = windows::Win32::System::Threading::GetCurrentThreadId();
        let _ = tx.send(thread_id);

        let on_change_ptr = Arc::into_raw(on_change) as isize;
        windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
            hwnd,
            windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            on_change_ptr,
        );

        let _ = AddClipboardFormatListener(hwnd);

        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if !ret.as_bool() {
                break; // WM_QUIT（stop()から送られる）
            }
            if msg.message == WM_CLIPBOARDUPDATE {
                handle_clipboard_update(&on_change_ptr);
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = RemoveClipboardFormatListener(hwnd);
        let _ = DestroyWindow(hwnd);
        // Arcの所有権を取り戻してdropする（into_rawで手放した分を回収）
        drop(Arc::from_raw(on_change_ptr as *const Box<dyn Fn(ClipboardSnapshot) + Send + Sync>));
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// `WM_CLIPBOARDUPDATE`受信時の実処理。除外規約チェック→内容読み取り→コールバック呼び出し。
fn handle_clipboard_update(on_change_ptr: &isize) {
    unsafe {
        if is_excluded_from_monitoring() {
            return;
        }

        let Ok(()) = OpenClipboard(None) else {
            return;
        };

        let snapshot = if IsClipboardFormatAvailable(CF_HDROP.0 as u32).is_ok() {
            read_file_paths()
        } else if IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_ok() {
            read_text()
        } else if IsClipboardFormatAvailable(CF_DIB.0 as u32).is_ok() {
            read_image_as_png()
        } else {
            None
        };

        let _ = CloseClipboard();

        if let Some(snapshot) = snapshot {
            let callback = &*(*on_change_ptr as *const Box<dyn Fn(ClipboardSnapshot) + Send + Sync>);
            callback(snapshot);
        }
    }
}

/// requirements.md 10.3章の除外規約チェック。
unsafe fn is_excluded_from_monitoring() -> bool {
    let format_name = to_wide(EXCLUDE_FORMAT_NAME);
    let format_id = RegisterClipboardFormatW(PCWSTR(format_name.as_ptr()));
    if format_id == 0 {
        return false;
    }
    IsClipboardFormatAvailable(format_id).is_ok()
}

unsafe fn read_text() -> Option<ClipboardSnapshot> {
    let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
    let ptr = GlobalLock(handle.into()) as *const u16;
    if ptr.is_null() {
        return None;
    }
    // NUL終端のUTF-16文字列として読み取る
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(ptr, len);
    let text = String::from_utf16_lossy(slice);
    let _ = GlobalUnlock(handle.into());
    Some(ClipboardSnapshot::Text(text))
}

unsafe fn read_file_paths() -> Option<ClipboardSnapshot> {
    let handle = GetClipboardData(CF_HDROP.0 as u32).ok()?;
    let hdrop = windows::Win32::UI::Shell::HDROP(handle.0 as *mut _);
    let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count {
        let len = DragQueryFileW(hdrop, i, None);
        let mut buf = vec![0u16; (len + 1) as usize];
        DragQueryFileW(hdrop, i, Some(&mut buf));
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        paths.push(PathBuf::from(path));
    }
    Some(ClipboardSnapshot::FilePaths(paths))
}

/// CF_DIB（デバイス非依存ビットマップ）をPNGへ変換する。
/// ヘッダ（BITMAPINFOHEADER）を読み取り、下から上へ格納されたBGRピクセルデータを
/// `image`クレートのRGBA8バッファへ詰め替える簡易実装。
unsafe fn read_image_as_png() -> Option<ClipboardSnapshot> {
    let handle = GetClipboardData(CF_DIB.0 as u32).ok()?;
    let hglobal = handle.into();
    let ptr = GlobalLock(hglobal) as *const u8;
    if ptr.is_null() {
        return None;
    }
    let size = GlobalSize(hglobal);

    // BITMAPINFOHEADER先頭のwidth/height/bitCountだけを最小限読み取る
    // （biSize:4, biWidth:4, biHeight:4, biPlanes:2, biBitCount:2 ... はオフセット固定）
    if size < 40 {
        let _ = GlobalUnlock(hglobal);
        return None;
    }
    let bytes = std::slice::from_raw_parts(ptr, size);
    let width = i32::from_le_bytes(bytes[4..8].try_into().unwrap()) as u32;
    let height_raw = i32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let bit_count = u16::from_le_bytes(bytes[14..16].try_into().unwrap());
    let height = height_raw.unsigned_abs();
    let top_down = height_raw < 0;

    if bit_count != 24 && bit_count != 32 {
        // 8bitパレット等は本実装では非対応（Phase2スコープの簡略化）
        let _ = GlobalUnlock(hglobal);
        return None;
    }

    let bytes_per_pixel = (bit_count / 8) as usize;
    let row_stride = ((width as usize * bytes_per_pixel + 3) / 4) * 4; // 4byte境界にパディング
    let pixel_data_offset = 40usize; // BITMAPINFOHEADERのサイズ分だけオフセット
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for y in 0..height as usize {
        let src_row = if top_down { y } else { height as usize - 1 - y };
        let row_start = pixel_data_offset + src_row * row_stride;
        for x in 0..width as usize {
            let src = row_start + x * bytes_per_pixel;
            if src + bytes_per_pixel > bytes.len() {
                continue;
            }
            let b = bytes[src];
            let g = bytes[src + 1];
            let r = bytes[src + 2];
            let a = if bytes_per_pixel == 4 { bytes[src + 3] } else { 255 };
            let dst = (y * width as usize + x) * 4;
            rgba[dst] = r;
            rgba[dst + 1] = g;
            rgba[dst + 2] = b;
            rgba[dst + 3] = a;
        }
    }
    let _ = GlobalUnlock(hglobal);

    let image = image::RgbaImage::from_raw(width, height, rgba)?;
    let mut png_bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut png_bytes, image::ImageFormat::Png)
        .ok()?;
    Some(ClipboardSnapshot::Image(png_bytes.into_inner()))
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
