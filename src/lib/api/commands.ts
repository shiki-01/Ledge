/**
 * invoke()の型付きラッパー。Rust側コマンド名と1:1対応させる（architecture.md 3章）。
 * コンポーネントから直接`invoke()`を呼ばず、必ずこのモジュール経由にする。
 */
import { invoke } from "@tauri-apps/api/core";
import type { ShelfItem } from "../types/shelf";
import type { AppSettings, AppSettingsPatch } from "../types/settings";
import type { ClipboardEntry } from "../types/clipboard";
import type { Tag } from "../types/tags";

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

/** 履歴取得（F-14: queryはLIKEエスケープ済みの検索、F-17: tagIdでタグ絞り込み） */
export function clipboardListHistory(query?: string, tagId?: number): Promise<ClipboardEntry[]> {
  return invoke<ClipboardEntry[]>("clipboard_list_history", { query, tagId });
}

/** 複数のテキストアイテムを改行結合し、新規テキストエントリとして記録する（F-15） */
export function clipboardStackEntries(ids: number[]): Promise<ClipboardEntry> {
  return invoke<ClipboardEntry>("clipboard_stack_entries", { ids });
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

/** タグ一覧取得（F-17） */
export function tagsList(): Promise<Tag[]> {
  return invoke<Tag[]>("tags_list");
}

/** タグ作成（name UNIQUE制約違反はShelfErrorのConflictとして返る） */
export function tagsCreate(name: string, color?: string): Promise<Tag> {
  return invoke<Tag>("tags_create", { name, color });
}

/** タグ削除（関連付けはON DELETE CASCADEで自動的に消える） */
export function tagsDelete(id: number): Promise<void> {
  return invoke<void>("tags_delete", { id });
}

/** 指定エントリのタグ付けを一括置き換えする */
export function clipboardSetTags(id: number, tagIds: number[]): Promise<void> {
  return invoke<void>("clipboard_set_tags", { id, tagIds });
}

/** 設定取得 */
export function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

/** 設定更新（ホットキー再登録をトリガする） */
export function updateSettings(patch: AppSettingsPatch): Promise<AppSettings> {
  return invoke<AppSettings>("update_settings", { patch });
}
