/**
 * シェルフアイテムの種別。Rust側 `storage::models::ShelfItemType` と対応する。
 */
export type ShelfItemType = "file" | "folder";

/**
 * シェルフ内の1アイテム。Rust側 `storage::models::ShelfItem` とフィールド名（camelCase）を揃えている。
 */
export interface ShelfItem {
  id: number;
  itemType: ShelfItemType;
  sourcePath: string;
  displayName: string;
  sizeBytes: number | null;
  locked: boolean;
  sortOrder: number;
  addedAt: string;
  /** 元ファイル/フォルダが現在参照可能かどうか（一覧取得のたびにRust側で算出される） */
  missing: boolean;
  /** 元ファイルの最終更新日時（Unixエポックミリ秒）。取得できない場合はnull（F-07プレビュー用） */
  modifiedAtMs: number | null;
}
