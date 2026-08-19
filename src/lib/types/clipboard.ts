/**
 * クリップボード履歴アイテムの内容種別。Rust側 `storage::models::ClipboardContentType` と対応する。
 */
export type ClipboardContentType = "text" | "image" | "file_paths";

/**
 * クリップボード履歴の1件。Rust側 `storage::models::ClipboardEntry` とフィールド名（camelCase）を揃えている。
 * `contentHash`（DB上の重複排除キー）はフロントエンドに公開する必要が無いため含めていない。
 */
export interface ClipboardEntry {
  id: number;
  contentType: ClipboardContentType;
  textContent: string | null;
  imagePath: string | null;
  thumbnailPath: string | null;
  /** `contentType === "file_paths"` の場合のみ値を持つ */
  filePaths: string[] | null;
  pinned: boolean;
  createdAt: string;
  updatedAt: string;
}
