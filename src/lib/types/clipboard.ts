import type { Tag } from "./tags";

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
  /**
   * 重複排除用のcontent hash。F-22（デバイス間同期）でFirestoreドキュメントIDとして
   * そのまま流用するため公開している（`src/lib/sync/clipboardSync.ts`が使用、
   * architecture.md 10.2章。以前は「フロントに公開する必要が無い」として含めていなかった
   * フィールドだが、同期エンジン追加に伴い公開する方針に変更した）。
   */
  contentHash: string;
  /** このエントリに付与されたタグ（Phase4, F-17） */
  tags: Tag[];
}
