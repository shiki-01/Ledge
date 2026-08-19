<script lang="ts">
  /**
   * 設定画面（Phase3）。ホットキー・表示位置/透明度・自動起動・ドラッグ自動検知・
   * クリップボード自動クリア閾値を1画面にまとめる（要件定義書6章）。
   *
   * ホットキー以外の項目は変更のたびに即座に`updateSettings`を呼んで反映する
   * （architecture.md 8.2章の「設定変更は即座に反映する」方針に合わせた）。
   * ホットキーはテキスト入力の都度invokeすると入力中に誤登録が走るため、blur/Enter時にのみ
   * 反映する（迷った設計判断）。
   */
  import { settingsStore } from "../stores/settingsStore";
  import { updateSettings, syncSetFirebasePassword } from "../api/commands";
  import { isShelfErrorPayload } from "../types/error";
  import { testFirebaseConnection } from "../sync/firebase";
  import { startClipboardSync } from "../sync/clipboardSync";
  import type { AppSettingsPatch, FirebaseConfig, ShelfEdge } from "../types/settings";

  let errorMessage = $state<string | null>(null);
  let errorTimer: ReturnType<typeof setTimeout> | undefined;

  let hotkeyDraft = $state("");

  // F-22（デバイス間同期）: Firebase構成のテキスト入力はホットキーと同様blur/Enter確定にする
  // （入力の都度invokeすると打鍵中に何度も設定ファイルへ書き込むことになるため）。
  let firebaseApiKeyDraft = $state("");
  let firebaseAuthDomainDraft = $state("");
  let firebaseProjectIdDraft = $state("");
  let firebaseAppIdDraft = $state("");
  let firebaseEmailDraft = $state("");
  // パスワードは`AppSettings`（get_settingsのレスポンス）に一切含めない設計のため、ストアから
  // 復元することはできない。設定画面を開き直すたびに空欄から入力し直す想定（迷った設計判断:
  // 書き込み専用コマンドsyncSetFirebasePassword経由でのみ保存する。api/commands.ts参照）。
  let firebasePasswordDraft = $state("");

  let testingConnection = $state(false);
  let testResult = $state<{ ok: boolean; message: string } | null>(null);

  // ストアの値が変わるたび（初回取得・他画面からの変更どちらも）ドラフトを同期する
  $effect(() => {
    const current = $settingsStore;
    if (current) {
      hotkeyDraft = current.shelfHotkey;
      firebaseApiKeyDraft = current.firebaseApiKey ?? "";
      firebaseAuthDomainDraft = current.firebaseAuthDomain ?? "";
      firebaseProjectIdDraft = current.firebaseProjectId ?? "";
      firebaseAppIdDraft = current.firebaseAppId ?? "";
      firebaseEmailDraft = current.firebaseEmail ?? "";
    }
  });

  function showError(e: unknown): void {
    errorMessage = isShelfErrorPayload(e) ? e.message : "予期しないエラーが発生しました";
    clearTimeout(errorTimer);
    errorTimer = setTimeout(() => {
      errorMessage = null;
    }, 4000);
  }

  async function apply(patch: AppSettingsPatch): Promise<void> {
    try {
      await updateSettings(patch);
      await settingsStore.refresh();
    } catch (e) {
      showError(e);
    }
  }

  function handleHotkeySubmit(): void {
    if (!$settingsStore || hotkeyDraft === $settingsStore.shelfHotkey) return;
    void apply({ shelfHotkey: hotkeyDraft });
  }

  function handleEdgeChange(e: Event): void {
    const value = (e.target as HTMLSelectElement).value as ShelfEdge;
    void apply({ shelfEdge: value });
  }

  function handleOpacityChange(e: Event): void {
    const value = Number((e.target as HTMLInputElement).value);
    void apply({ opacity: value });
  }

  function handleAutostartChange(e: Event): void {
    const checked = (e.target as HTMLInputElement).checked;
    void apply({ autostartEnabled: checked });
  }

  function handleAutoShowChange(e: Event): void {
    const checked = (e.target as HTMLInputElement).checked;
    void apply({ autoShowOnDragStart: checked });
  }

  function handleMaxEntriesChange(e: Event): void {
    const value = Number((e.target as HTMLInputElement).value);
    if (Number.isNaN(value) || value < 1) return;
    void apply({ clipboardMaxEntries: Math.trunc(value) });
  }

  function handleRetentionDaysChange(e: Event): void {
    const value = Number((e.target as HTMLInputElement).value);
    if (Number.isNaN(value) || value < 1) return;
    void apply({ clipboardRetentionDays: Math.trunc(value) });
  }

  function handleSyncEnabledChange(e: Event): void {
    const checked = (e.target as HTMLInputElement).checked;
    void apply({ syncEnabled: checked });
  }

  function handleFirebaseApiKeySubmit(): void {
    if (!$settingsStore || firebaseApiKeyDraft === ($settingsStore.firebaseApiKey ?? "")) return;
    void apply({ firebaseApiKey: firebaseApiKeyDraft });
  }

  function handleFirebaseAuthDomainSubmit(): void {
    if (!$settingsStore || firebaseAuthDomainDraft === ($settingsStore.firebaseAuthDomain ?? "")) return;
    void apply({ firebaseAuthDomain: firebaseAuthDomainDraft });
  }

  function handleFirebaseProjectIdSubmit(): void {
    if (!$settingsStore || firebaseProjectIdDraft === ($settingsStore.firebaseProjectId ?? "")) return;
    void apply({ firebaseProjectId: firebaseProjectIdDraft });
  }

  function handleFirebaseAppIdSubmit(): void {
    if (!$settingsStore || firebaseAppIdDraft === ($settingsStore.firebaseAppId ?? "")) return;
    void apply({ firebaseAppId: firebaseAppIdDraft });
  }

  function handleFirebaseEmailSubmit(): void {
    if (!$settingsStore || firebaseEmailDraft === ($settingsStore.firebaseEmail ?? "")) return;
    void apply({ firebaseEmail: firebaseEmailDraft });
  }

  async function handleFirebasePasswordSubmit(): Promise<void> {
    if (firebasePasswordDraft === "") return;
    try {
      await syncSetFirebasePassword(firebasePasswordDraft);
    } catch (e) {
      showError(e);
    }
  }

  /** 「接続テスト」ボタン: 現在ドラフト中の構成・Email/Passwordでサインインを試みる（F-22）。 */
  async function handleTestConnection(): Promise<void> {
    testResult = null;
    if (
      !firebaseApiKeyDraft ||
      !firebaseAuthDomainDraft ||
      !firebaseProjectIdDraft ||
      !firebaseAppIdDraft ||
      !firebaseEmailDraft ||
      !firebasePasswordDraft
    ) {
      testResult = { ok: false, message: "Firebase構成・Email・Passwordをすべて入力してください" };
      return;
    }
    testingConnection = true;
    try {
      const config: FirebaseConfig = {
        apiKey: firebaseApiKeyDraft,
        authDomain: firebaseAuthDomainDraft,
        projectId: firebaseProjectIdDraft,
        appId: firebaseAppIdDraft,
      };
      const result = await testFirebaseConnection(config, firebaseEmailDraft, firebasePasswordDraft);
      testResult = result.ok
        ? { ok: true, message: "接続に成功しました" }
        : { ok: false, message: `接続に失敗しました（${result.errorCode}）: ${result.message}` };

      // 接続テスト成功時点のサインインをそのまま同期エンジンの常駐セッションにする
      // （architecture.md 10.2章「認証セッションの持ち方」）。同期エンジンの開始自体は
      // App.svelte側でも`syncEnabled`と構成の揃い具合を見て起動するが、既に`syncEnabled`が
      // ONの状態で構成だけ直していた場合、設定変更を経ずにこの場で即座にサインイン後の
      // セッションを使い始められるようここでも起動しておく（迷った設計判断: 呼び出し元へ報告）。
      if (result.ok && $settingsStore?.syncEnabled) {
        void startClipboardSync(config);
      }
    } finally {
      testingConnection = false;
    }
  }
</script>

<div class="settings">
  <header class="settings__header">
    <span class="settings__title">設定</span>
  </header>

  {#if errorMessage}
    <div class="settings__toast" role="alert">{errorMessage}</div>
  {/if}

  {#if $settingsStore}
    {@const s = $settingsStore}
    <div class="settings__body">
      <section class="settings__section">
        <h3 class="settings__section-title">ホットキー（F-20）</h3>
        <div class="settings__row">
          <label class="settings__label" for="shelf-hotkey">シェルフ表示/非表示</label>
          <input
            id="shelf-hotkey"
            class="settings__input"
            type="text"
            bind:value={hotkeyDraft}
            onblur={handleHotkeySubmit}
            onkeydown={(e) => {
              if (e.key === "Enter") handleHotkeySubmit();
            }}
          />
        </div>
        <p class="settings__hint">例: Ctrl+Alt+S / Cmd+Option+S</p>
      </section>

      <section class="settings__section">
        <h3 class="settings__section-title">表示位置/透明度（F-10）</h3>
        <div class="settings__row">
          <label class="settings__label" for="shelf-edge">表示位置</label>
          <select id="shelf-edge" class="settings__input" value={s.shelfEdge} onchange={handleEdgeChange}>
            <option value="right">右</option>
            <option value="left">左</option>
            <option value="top">上</option>
            <option value="bottom">下</option>
          </select>
        </div>
        <div class="settings__row">
          <label class="settings__label" for="shelf-opacity">
            透明度（{Math.round(s.opacity * 100)}%）
          </label>
          <input
            id="shelf-opacity"
            class="settings__input"
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={s.opacity}
            oninput={handleOpacityChange}
          />
        </div>
      </section>

      <section class="settings__section">
        <h3 class="settings__section-title">自動起動/自動表示（F-19, F-08）</h3>
        <div class="settings__row settings__row--checkbox">
          <label>
            <input type="checkbox" checked={s.autostartEnabled} onchange={handleAutostartChange} />
            OS起動時に自動起動する
          </label>
        </div>
        <div class="settings__row settings__row--checkbox">
          <label>
            <input type="checkbox" checked={s.autoShowOnDragStart} onchange={handleAutoShowChange} />
            ドラッグ開始を検知してシェルフを自動表示する（Windows先行）
          </label>
        </div>
      </section>

      <section class="settings__section">
        <h3 class="settings__section-title">クリップボード履歴の自動クリア（F-16）</h3>
        <div class="settings__row">
          <label class="settings__label" for="clipboard-max-entries">件数上限</label>
          <input
            id="clipboard-max-entries"
            class="settings__input"
            type="number"
            min="1"
            value={s.clipboardMaxEntries}
            onchange={handleMaxEntriesChange}
          />
        </div>
        <div class="settings__row">
          <label class="settings__label" for="clipboard-retention-days">経過日数上限</label>
          <input
            id="clipboard-retention-days"
            class="settings__input"
            type="number"
            min="1"
            value={s.clipboardRetentionDays}
            onchange={handleRetentionDaysChange}
          />
        </div>
      </section>

      <section class="settings__section">
        <h3 class="settings__section-title">デバイス間同期（F-22, Bring Your Own Firebase）</h3>
        <div class="settings__row settings__row--checkbox">
          <label>
            <input type="checkbox" checked={s.syncEnabled} onchange={handleSyncEnabledChange} />
            デバイス間同期を有効にする
          </label>
        </div>
        <div class="settings__row">
          <label class="settings__label" for="firebase-api-key">Firebase apiKey</label>
          <input
            id="firebase-api-key"
            class="settings__input"
            type="text"
            bind:value={firebaseApiKeyDraft}
            onblur={handleFirebaseApiKeySubmit}
            onkeydown={(e) => {
              if (e.key === "Enter") handleFirebaseApiKeySubmit();
            }}
          />
        </div>
        <div class="settings__row">
          <label class="settings__label" for="firebase-auth-domain">Firebase authDomain</label>
          <input
            id="firebase-auth-domain"
            class="settings__input"
            type="text"
            bind:value={firebaseAuthDomainDraft}
            onblur={handleFirebaseAuthDomainSubmit}
            onkeydown={(e) => {
              if (e.key === "Enter") handleFirebaseAuthDomainSubmit();
            }}
          />
        </div>
        <div class="settings__row">
          <label class="settings__label" for="firebase-project-id">Firebase projectId</label>
          <input
            id="firebase-project-id"
            class="settings__input"
            type="text"
            bind:value={firebaseProjectIdDraft}
            onblur={handleFirebaseProjectIdSubmit}
            onkeydown={(e) => {
              if (e.key === "Enter") handleFirebaseProjectIdSubmit();
            }}
          />
        </div>
        <div class="settings__row">
          <label class="settings__label" for="firebase-app-id">Firebase appId</label>
          <input
            id="firebase-app-id"
            class="settings__input"
            type="text"
            bind:value={firebaseAppIdDraft}
            onblur={handleFirebaseAppIdSubmit}
            onkeydown={(e) => {
              if (e.key === "Enter") handleFirebaseAppIdSubmit();
            }}
          />
        </div>
        <div class="settings__row">
          <label class="settings__label" for="firebase-email">サインインEmail</label>
          <input
            id="firebase-email"
            class="settings__input"
            type="text"
            bind:value={firebaseEmailDraft}
            onblur={handleFirebaseEmailSubmit}
            onkeydown={(e) => {
              if (e.key === "Enter") handleFirebaseEmailSubmit();
            }}
          />
        </div>
        <div class="settings__row">
          <label class="settings__label" for="firebase-password">サインインPassword</label>
          <input
            id="firebase-password"
            class="settings__input"
            type="password"
            bind:value={firebasePasswordDraft}
            onblur={handleFirebasePasswordSubmit}
            onkeydown={(e) => {
              if (e.key === "Enter") void handleFirebasePasswordSubmit();
            }}
          />
        </div>
        <div class="settings__row">
          <button
            type="button"
            class="settings__button"
            disabled={testingConnection}
            onclick={handleTestConnection}
          >
            {testingConnection ? "接続確認中…" : "接続テスト"}
          </button>
        </div>
        {#if testResult}
          <p class="settings__hint" class:settings__hint--error={!testResult.ok}>
            {testResult.message}
          </p>
        {/if}
        <p class="settings__hint">
          Firebase Consoleで自分のプロジェクトを作成し、Firestore と
          Authentication（Email/Password）を有効化してください。構成情報とサインインアカウントの
          作成方法は architecture.md 10.2章を参照してください。
        </p>
      </section>
    </div>
  {/if}
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    height: 100%;
    box-sizing: border-box;
    padding: 0.6rem;
    gap: 0.5rem;
    color: #f5f5f5;
  }

  .settings__header {
    display: flex;
    align-items: center;
  }

  .settings__title {
    font-size: 0.85rem;
    font-weight: 600;
  }

  .settings__toast {
    background: rgba(220, 38, 38, 0.85);
    color: #fff;
    font-size: 0.75rem;
    padding: 0.4rem 0.6rem;
    border-radius: 4px;
  }

  .settings__body {
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
  }

  .settings__section-title {
    margin: 0 0 0.4rem;
    font-size: 0.72rem;
    font-weight: 600;
    opacity: 0.75;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }

  .settings__row {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 0.5rem;
    font-size: 0.78rem;
  }

  .settings__row--checkbox {
    flex-direction: row;
    align-items: center;
  }

  .settings__row--checkbox label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .settings__label {
    opacity: 0.85;
  }

  .settings__input {
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: inherit;
    border-radius: 4px;
    padding: 0.3rem 0.4rem;
    font: inherit;
  }

  .settings__hint {
    margin: 0.2rem 0 0;
    font-size: 0.68rem;
    opacity: 0.55;
  }

  .settings__hint--error {
    opacity: 0.9;
    color: #f87171;
  }

  .settings__button {
    align-self: flex-start;
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: inherit;
    border-radius: 4px;
    padding: 0.35rem 0.7rem;
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .settings__button:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
