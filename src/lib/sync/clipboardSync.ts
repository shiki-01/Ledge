/**
 * F-22（デバイス間同期）: 「ピン留め済みテキストのクリップボード履歴」に絞ったFirestore実同期
 * （push/pull・onSnapshotによるリアルタイムリスナー）本体（architecture.md 10.2章）。
 *
 * 同期対象は`contentType === "text" && pinned === true`のクリップボード履歴のみ。
 * shelf_items、画像・ファイルパスのピン留めはスコープ外（architecture.md 10.2章）。
 */
import {
  getFirestore,
  collection,
  doc,
  onSnapshot,
  writeBatch,
  type DocumentChange,
  type DocumentData,
  type Firestore,
  type Unsubscribe,
} from "firebase/firestore";
import { getAuth } from "firebase/auth";
import type { FirebaseApp } from "firebase/app";
import { initFirebase } from "./firebase";
import { clipboardListHistory, clipboardSyncUpsertFromCloud, clipboardSyncUnpinByHash } from "../api/commands";
import type { FirebaseConfig } from "../types/settings";

/** Firestore上のドキュメント1件の形（`users/{uid}/clipboard_history/{contentHash}`）。 */
interface CloudClipboardDoc {
  textContent: string;
  updatedAt: string;
}

/**
 * pushの差分計算の基準となる「前回同期した`content_hash` → `updatedAt`」のローカルキャッシュ。
 *
 * architecture.md 10.2章では`tauri-plugin-store`の別キーに保持する案を挙げていたが、
 * フロント側には`@tauri-apps/plugin-store`（JS版store、Rust側のcrateとは別パッケージ）が
 * まだ導入されておらず、新規にnpm依存を増やすほどの内容でもないため、TauriのWebViewが持つ
 * `localStorage`をそのまま使うことにした（迷った設計判断: 呼び出し元へ報告）。あくまで同期
 * エンジン内部の実装詳細のキャッシュでありユーザー設定ではないため、`AppSettings`
 * （Rust側の構造体・`settings.json`）には含めない。消えても次回pushで全件差分として
 * 送り直されるだけで実害はない。
 */
const PUSH_CACHE_KEY = "ledge-sync-pushed-hashes";

/** contentHash -> 直近でクラウドと同期済みのupdatedAt */
type PushCache = Record<string, string>;

function loadPushCache(): PushCache {
  try {
    const raw = localStorage.getItem(PUSH_CACHE_KEY);
    return raw ? (JSON.parse(raw) as PushCache) : {};
  } catch {
    return {};
  }
}

function savePushCache(cache: PushCache): void {
  try {
    localStorage.setItem(PUSH_CACHE_KEY, JSON.stringify(cache));
  } catch {
    // 保存に失敗しても致命的ではない（次回push時に全件差分として送り直されるだけ）
  }
}

let unsubscribeSnapshot: Unsubscribe | undefined;
let currentApp: FirebaseApp | undefined;
let currentUid: string | undefined;

/**
 * pull（クラウド→ローカル反映）処理中はtrueになる。
 *
 * pull→ローカル反映→`clipboard://history-changed`→push→…という無限ループを防ぐため、
 * App.svelte側の`history-changed`購読はこのフラグを見て、pull由来の変更をそのままpushし
 * 返さないようにする（迷った設計判断: 呼び出し元へ報告）。
 *
 * このフラグだけで安全と言えるのは、Tauriの`emit`がRustコマンド内でコマンドの戻り値を
 * 返すより前に呼ばれ、かつIPCメッセージの到着順序が送信順どおりに保たれる前提があるため
 * （`clipboard_sync_upsert_from_cloud`等の中で`notify_history_changed`が先に呼ばれてから
 * `Ok(())`が返る）。この前提が崩れる場合（フロントの`await`が先に解決してフラグを戻して
 * しまう等）は、pull由来の変更が誤ってpushされる可能性が残る点は認識している。
 */
let applyingRemoteChange = false;

/** pull処理中かどうか。App.svelteから参照する。 */
export function isApplyingRemoteChange(): boolean {
  return applyingRemoteChange;
}

/**
 * 同期エンジンを開始する。常駐FirebaseAppからFirestoreを取得し、
 * `users/{uid}/clipboard_history`コレクションへのリアルタイムリスナーを張る。
 *
 * 未サインイン（「接続テスト」未実施、またはセッション失効）の場合は何もせず、警告を出す
 * のみに留める（architecture.md 10.2章の通り、このエンジン自体はサインインを行わない。
 * サインインは「接続テスト」実行時に済ませておく設計）。
 *
 * 既に開始済みの場合は一旦停止してから開始し直す（設定変更等での多重登録を防ぐ）。
 */
export async function startClipboardSync(config: FirebaseConfig): Promise<Unsubscribe> {
  stopClipboardSync();

  const app = initFirebase(config);
  const auth = getAuth(app);
  // `browserLocalPersistence`からのセッション復元は非同期のため、初期化直後は永続化された
  // サインイン済みセッションがあっても`auth.currentUser`がまだ`null`のことがある
  // （Firebase Auth SDKの既知の挙動）。`authStateReady()`で初回の状態解決を待ってから
  // `currentUser`を読むことで、アプリ再起動直後でも常駐セッションを正しく検出できるようにする。
  await auth.authStateReady();
  const uid = auth.currentUser?.uid;
  if (!uid) {
    console.warn(
      "[Ledge sync] 未サインインのため同期を開始できません。設定画面で「接続テスト」を行ってください。",
    );
    return () => {};
  }

  currentApp = app;
  currentUid = uid;

  const db = getFirestore(app);
  const historyCollection = collection(db, "users", uid, "clipboard_history");

  unsubscribeSnapshot = onSnapshot(historyCollection, (snapshot) => {
    void applyRemoteChanges(snapshot.docChanges());
  });

  // 同期無効化中に行われたローカルのピン留め変更を取りこぼさないよう、開始時に一度pushしておく
  void pushPinnedTextEntries();

  return () => {
    stopClipboardSync();
  };
}

async function applyRemoteChanges(changes: DocumentChange<DocumentData>[]): Promise<void> {
  if (changes.length === 0) return;

  applyingRemoteChange = true;
  try {
    const cache = loadPushCache();
    for (const change of changes) {
      const contentHash = change.doc.id;
      if (change.type === "removed") {
        await clipboardSyncUnpinByHash(contentHash);
        delete cache[contentHash];
        continue;
      }
      // Firestore側のドキュメント形は`CloudClipboardDoc`前提（`pushPinnedTextEntries`が書き込む形）
      const data = change.doc.data() as CloudClipboardDoc;
      await clipboardSyncUpsertFromCloud(contentHash, data.textContent, data.updatedAt);
      // pull由来の内容は既にクラウドと同期済みとみなし、次回pushで無駄な書き戻しをしない
      cache[contentHash] = data.updatedAt;
    }
    savePushCache(cache);
  } finally {
    applyingRemoteChange = false;
  }
}

/**
 * その時点でのピン留め済みテキスト一覧をFirestoreへ反映する（ローカル→クラウド、push）。
 * 前回push/pull時点の`content_hash`集合（`loadPushCache`）との差分のみを`writeBatch`で送る。
 *
 * 同期が開始されていない（`startClipboardSync`未実行、または未サインインで開始に失敗した）
 * 場合は何もしない。
 */
export async function pushPinnedTextEntries(): Promise<void> {
  if (!currentApp || !currentUid) return;

  const db: Firestore = getFirestore(currentApp);
  const historyCollection = collection(db, "users", currentUid, "clipboard_history");

  const entries = await clipboardListHistory();
  const pinnedTextEntries = entries.filter(
    (e) => e.contentType === "text" && e.pinned && e.textContent !== null,
  );

  const currentMap: PushCache = {};
  for (const entry of pinnedTextEntries) {
    currentMap[entry.contentHash] = entry.updatedAt;
  }

  const previous = loadPushCache();
  const batch = writeBatch(db);
  let hasChanges = false;

  for (const entry of pinnedTextEntries) {
    if (previous[entry.contentHash] !== entry.updatedAt) {
      batch.set(doc(historyCollection, entry.contentHash), {
        textContent: entry.textContent as string,
        updatedAt: entry.updatedAt,
      });
      hasChanges = true;
    }
  }
  for (const hash of Object.keys(previous)) {
    if (!(hash in currentMap)) {
      batch.delete(doc(historyCollection, hash));
      hasChanges = true;
    }
  }

  if (hasChanges) {
    await batch.commit();
  }
  savePushCache(currentMap);
}

/** 同期エンジンを停止する。リスナーを解除し、以後`pushPinnedTextEntries`も何もしなくなる。 */
export function stopClipboardSync(): void {
  unsubscribeSnapshot?.();
  unsubscribeSnapshot = undefined;
  currentApp = undefined;
  currentUid = undefined;
}
