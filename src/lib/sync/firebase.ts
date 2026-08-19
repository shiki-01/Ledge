/**
 * F-22（デバイス間同期）: Bring Your Own Firebase方式の常駐セッション管理・疎通確認ロジック
 * （architecture.md 10.2章）。
 *
 * 同期ロジックはOS非依存のため、Rust側（src-tauri/）ではなくフロントエンドに置く方針
 * （architecture.md 10.2章「実装配置」）。Firebase Web SDKはTauriのWebViewから直接利用する。
 *
 * 実際のFirestore push/pull（onSnapshotによるリアルタイムリスナー含む）は
 * `src/lib/sync/clipboardSync.ts`に置く。本ファイルはFirebaseAppの初期化・
 * サインイン疎通確認のみを担う。
 */
import { initializeApp, getApps, type FirebaseApp } from "firebase/app";
import { getAuth, signInWithEmailAndPassword, type AuthError } from "firebase/auth";
import type { FirebaseConfig } from "../types/settings";

/**
 * 同期エンジンが使い続ける常駐FirebaseAppの固定名。
 *
 * 以前は疎通確認のたびにユニーク名の一時的なFirebaseAppを作って確認後に`deleteApp`する実装
 * だったが、実際の同期エンジンは常駐して`onSnapshot`を張り続ける必要があり、アプリ再起動の
 * たびにパスワード入力を求めるのは非現実的（パスワードは書き込み専用コマンドで保存するのみで
 * `get_settings`からは読み出せない設計にしているため、そもそも再入力なしでは再サインイン
 * できない）。そこでFirebase Auth SDKの既定の永続化（`browserLocalPersistence`。TauriのWebView
 * はブラウザ相当のlocalStorage/IndexedDBを持つため、サインイン状態はアプリ再起動後も保持される）
 * を利用し、「接続テスト」に成功した時点のサインインをそのまま同期エンジンが使う常駐セッション
 * とする方式に変更した（architecture.md 10.2章「認証セッションの持ち方」）。
 */
const SYNC_APP_NAME = "ledge-sync";

/**
 * 固定名の常駐FirebaseAppを取得する。既に初期化済みならそれをそのまま返し（多重初期化は
 * エラーになるため）、無ければ新規初期化する。`deleteApp`は呼ばない。
 */
export function initFirebase(config: FirebaseConfig): FirebaseApp {
  const existing = getApps().find((app) => app.name === SYNC_APP_NAME);
  if (existing) {
    return existing;
  }
  return initializeApp(config, SYNC_APP_NAME);
}

/** `testFirebaseConnection`の結果。 */
export type FirebaseConnectionResult =
  | { ok: true }
  | { ok: false; errorCode: string; message: string };

/**
 * 指定のFirebase構成でAppを初期化（または既存の常駐Appを再利用）し、Email/Passwordでの
 * サインインを試みる。設定画面の「接続テスト」ボタンから呼ばれる想定。
 *
 * ここでのサインインは疎通確認だけでなく、成功時点のセッションがそのまま同期エンジンの
 * 常駐セッションになる（`browserLocalPersistence`により永続化される）。そのため、以前の
 * 実装と異なりAppを都度破棄することはしない。
 */
export async function testFirebaseConnection(
  config: FirebaseConfig,
  email: string,
  password: string,
): Promise<FirebaseConnectionResult> {
  const app = initFirebase(config);
  try {
    const auth = getAuth(app);
    await signInWithEmailAndPassword(auth, email, password);
    return { ok: true };
  } catch (e) {
    const authError = e as AuthError;
    return {
      ok: false,
      errorCode: authError.code ?? "unknown",
      message: authError.message ?? "接続テストに失敗しました",
    };
  }
}
