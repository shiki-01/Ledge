/**
 * invoke()の型付きラッパー。Rust側コマンド名と1:1対応させる（architecture.md 3章）。
 * コンポーネントから直接`invoke()`を呼ばず、必ずこのモジュール経由にする。
 */
import { invoke } from "@tauri-apps/api/core";
import type { ShelfItem } from "../types/shelf";
import type { AppSettings, AppSettingsPatch } from "../types/settings";

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

/** 一括削除（F-06実装後はロック対象を除外する） */
export function shelfClear(excludeLocked: boolean): Promise<void> {
  return invoke<void>("shelf_clear", { excludeLocked });
}

/** シェルフ→外部アプリへのネイティブドラッグ開始（F-03） */
export function shelfBeginDragOut(ids: number[]): Promise<void> {
  return invoke<void>("shelf_begin_drag_out", { ids });
}

/** 設定取得 */
export function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

/** 設定更新（ホットキー再登録をトリガする） */
export function updateSettings(patch: AppSettingsPatch): Promise<AppSettings> {
  return invoke<AppSettings>("update_settings", { patch });
}
