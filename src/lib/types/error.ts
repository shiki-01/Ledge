/**
 * Rust側 `error::ShelfError` がシリアライズされた形（architecture.md 6章）。
 * Tauriコマンドが失敗した際、invoke()のPromiseはこの形の値でreject（catch）される。
 */
export interface ShelfErrorPayload {
  code: string;
  message: string;
}

/** invoke()の失敗時に受け取った値がShelfErrorPayload形かどうかを判定する。 */
export function isShelfErrorPayload(value: unknown): value is ShelfErrorPayload {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "message" in value &&
    typeof (value as { message: unknown }).message === "string"
  );
}
