/**
 * よく使うフォルダの1件。Rust側 `storage::models::FavoriteFolder` とフィールド名（camelCase）を揃えている
 * （Phase6, F-09）。
 */
export interface FavoriteFolder {
  id: number;
  folderPath: string;
  displayName: string;
  sortOrder: number;
  addedAt: string;
  /** フォルダが現在参照可能かどうか（一覧取得のたびにRust側で算出される） */
  missing: boolean;
}
