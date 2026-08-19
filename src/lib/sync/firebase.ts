/**
 * F-22（デバイス間同期）: Bring Your Own Firebase方式の疎通確認ロジック（architecture.md 10.2章）。
 *
 * 実際のFirestore同期（shelf_items / clipboard_historyのpush/pull・onSnapshotによる
 * リアルタイムリスナー）は本ファイルのスコープに含めない（次フェーズで着手）。
 * ここでは「ユーザーが設定画面に入力したFirebase構成・Email/Passwordで実際にサインインできるか」の
 * 疎通確認（`testFirebaseConnection`）のみを提供する。
 *
 * 同期ロジックはOS非依存のため、Rust側（src-tauri/）ではなくフロントエンドに置く方針
 * （architecture.md 10.2章「実装配置」）。Firebase Web SDKはTauriのWebViewから直接利用する。
 */
import { initializeApp, deleteApp, type FirebaseApp } from "firebase/app";
import { getAuth, signInWithEmailAndPassword, type AuthError } from "firebase/auth";
import type { FirebaseConfig } from "../types/settings";

/**
 * Firebase Appを初期化する。呼び出しのたびに新しいFirebaseAppインスタンスを作る
 * （`initializeApp`はデフォルト名では多重初期化するとエラーになるため、疎通確認等の
 * 一時利用ではユニークな名前を付けて都度破棄する運用にしている。後続フェーズで常駐の
 * 同期セッションを持つ場合は、この関数とは別に「アプリ全体で1つだけ持つFirebaseApp」を
 * 管理する仕組みが必要になる想定。今回のスコープでは疎通確認用途のみ）。
 */
export function initFirebase(config: FirebaseConfig): FirebaseApp {
  const uniqueName = `ledge-sync-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return initializeApp(config, uniqueName);
}

/** `testFirebaseConnection`の結果。 */
export type FirebaseConnectionResult =
  | { ok: true }
  | { ok: false; errorCode: string; message: string };

/**
 * 指定のFirebase構成でAppを初期化し、Email/PasswordでのサインインをtestFirebaseConnectionという
 * 名の通り「疎通確認」のためだけに1回試みる。設定画面の「接続テスト」ボタンから呼ばれる想定。
 * 成功/失敗いずれの場合も、確認用に作った一時的なFirebaseAppは後始末として破棄する。
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
  } finally {
    // 疎通確認専用の一時Appなので、結果に関わらず破棄してリソースを残さない
    await deleteApp(app).catch(() => {
      // 破棄失敗は疎通確認結果に影響しないため無視する
    });
  }
}
