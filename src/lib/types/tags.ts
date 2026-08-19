/**
 * タグ（Phase4, F-17）。Rust側 `storage::models::Tag` とフィールド名（camelCase）を揃えている。
 */
export interface Tag {
  id: number;
  name: string;
  /** 自由入力のhexカラーコード文字列（`#RRGGBB`）。DB制約は無い（architecture.md 9.3章）。 */
  color: string | null;
}
