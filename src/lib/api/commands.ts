/**
 * invoke()の型付きラッパー。Rust側コマンド名と1:1対応させる（architecture.md 3章）。
 * コンポーネントから直接`invoke()`を呼ばず、必ずこのモジュール経由にする。
 */
import { invoke } from "@tauri-apps/api/core";
import type { ShelfItem } from "../types/shelf";
import type { AppSettings, AppSettingsPatch } from "../types/settings";
import type { ClipboardEntry } from "../types/clipboard";

/** 起動時・表示時の一覧取得 */
export function shelfListItems(): Promise<ShelfItem[]> {
  return invoke<ShelfItem[]>("shelf_list_items");
}

/** D&Dで受け取ったパスを格納する（追加分を返す） */
export function shelfAddPaths(paths: string[]): Promise<ShelfItem[]> {
  return invoke<ShelfItem[]>("shelf_add_paths", { paths });
}

/** 個別削除 */
export function shelfRemoveItem(id: number): Promise<void> {
  return invoke<void>("shelf_remove_item", { id });
}

/** ロック状態の変更（F-06。ロック中は「全て削除」の対象から除外される） */
export function shelfSetLocked(id: number, locked: boolean): Promise<void> {
  return invoke<void>("shelf_set_locked", { id, locked });
}

/** 一括削除（ロック対象を除外するかどうか。F-06実装後はデフォルトでtrueを渡す） */
export function shelfClear(excludeLocked: boolean): Promise<void> {
  return invoke<void>("shelf_clear", { excludeLocked });
}

/** シェルフ→外部アプリへのネイティブドラッグ開始（F-03） */
export function shelfBeginDragOut(ids: number[]): Promise<void> {
  return invoke<void>("shelf_begin_drag_out", { ids });
}

/** 履歴取得（F-14の本格検索はPhase4だが、queryは簡易LIKE検索として先行して渡せる） */
export function clipboardListHistory(query?: string): Promise<ClipboardEntry[]> {
  return invoke<ClipboardEntry[]>("clipboard_list_history", { query });
}

/** クリップボードへ書き戻す（F-12） */
export function clipboardPasteToActive(id: number): Promise<void> {
  return invoke<void>("clipboard_paste_to_active", { id });
}

/** ピン留め状態を変更する（F-13） */
export function clipboardSetPinned(id: number, pinned: boolean): Promise<void> {
  return invoke<void>("clipboard_set_pinned", { id, pinned });
}

/** 個別削除 */
export function clipboardDelete(id: number): Promise<void> {
  return invoke<void>("clipboard_delete", { id });
}

/** 一括削除（ピン留めアイテムを除外するかどうか） */
export function clipboardClear(excludePinned: boolean): Promise<void> {
  return invoke<void>("clipboard_clear", { excludePinned });
}

/** 設定取得 */
export function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

/** 設定更新（ホットキー再登録をトリガする） */
export function updateSettings(patch: AppSettingsPatch): Promise<AppSettings> {
  return invoke<AppSettings>("update_settings", { patch });
}
