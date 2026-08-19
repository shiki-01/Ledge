/**
 * シェルフを表示する画面端。Rust側 `settings::ShelfEdge` と対応する。
 */
export type ShelfEdge = "top" | "bottom" | "left" | "right";

/**
 * アプリ設定。Rust側 `settings::AppSettings` とフィールド名を揃えている。
 */
export interface AppSettings {
  shelfHotkey: string;
  shelfEdge: ShelfEdge;
  /** クリップボード履歴の自動クリア（F-16）: 件数上限。既定500件。 */
  clipboardMaxEntries: number;
  /** クリップボード履歴の自動クリア（F-16）: 経過日数上限。既定30日。 */
  clipboardRetentionDays: number;
  /** シェルフ背景の透明度（F-10）。0.0（完全透明）〜1.0（不透明）、既定0.85。 */
  opacity: number;
  /** OS起動時の自動起動ON/OFF（F-19）。既定false。 */
  autostartEnabled: boolean;
  /** ドラッグ開始検知によるシェルフ自動表示のON/OFF（F-08 Windows先行）。既定true。 */
  autoShowOnDragStart: boolean;
}

/** `update_settings`コマンドへ渡す部分更新オブジェクト。 */
export type AppSettingsPatch = Partial<AppSettings>;
