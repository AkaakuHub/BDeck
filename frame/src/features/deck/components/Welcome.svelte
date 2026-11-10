<script lang="ts">
  import { Tick02Icon } from "@hugeicons/core-free-icons";
  import { deck } from "$domain/deck";
  import Icon from "$shared/ui/Icon";
  import icon from "../../../../../desktop/icons/128x128@2x.png";
  const hasAccount = $derived(deck.readyAccounts().length > 0);
  const needsLogin = $derived(!hasAccount && (deck.state()?.accounts.length ?? 0) > 0);
</script>
<section class="welcome" aria-labelledby="welcome-title">
  <div class="welcome-card">
    <img class="app-icon" src={icon} width="64" height="64" alt="" />
    <h1 id="welcome-title">{hasAccount ? "最初のカラムを追加" : needsLogin ? "Xにログインしましょう" : "BDeckへようこそ"}</h1>
    <p>{hasAccount ? "左の＋から、ホームや通知のカラムを追加できます。" : needsLogin ? "右上のアカウントから、Xにログインしてください。" : "Xのホームや通知を、ひとつの画面に。"}<br />{hasAccount ? "カラムはあとから追加・並べ替えできます。" : needsLogin ? "ログイン後に、カラムの追加や投稿ができます。" : "アカウントごとに、見たい情報を並べられます。"}</p>
    <ol aria-label="はじめるまでの手順">
      <li class:completed={hasAccount} aria-current={!hasAccount ? "step" : undefined}>
        <span class="step-number">{#if hasAccount}<Icon icon={Tick02Icon} size={14} />{:else}1{/if}</span>
        Xにログイン
      </li>
      <li class:upcoming={!hasAccount} aria-current={hasAccount ? "step" : undefined}>
        <span class="step-number">2</span>カラムを追加
      </li>
    </ol>
  </div>
</section>
<style>
  .welcome { position: absolute; inset: 48px 0 0 56px; display: grid; place-items: center; padding: 32px; box-sizing: border-box; overflow-y: auto; color: var(--color-text); }
  .welcome-card { width: min(100%, 432px); box-sizing: border-box; padding: 36px 32px 32px; text-align: center; border: 1px solid var(--color-border-strong); border-radius: 28px; background: linear-gradient(160deg, color-mix(in srgb, var(--color-surface) 90%, transparent), color-mix(in srgb, var(--color-surface) 50%, transparent)); box-shadow: 0 24px 64px -32px rgb(0 0 0 / 0.2), 0 4px 12px -6px rgb(0 0 0 / 0.08), inset 0 1px 0 var(--color-control); }
  .app-icon { display: block; margin: 0 auto 24px; width: 64px; height: 64px; filter: drop-shadow(0 8px 12px rgb(0 0 0 / 0.12)); }
  h1 { margin: 0; font-size: 25px; line-height: 1.4; font-weight: 600; letter-spacing: -0.035em; }
  p { margin: 12px 0 0; font-size: 13px; line-height: 1.85; color: var(--color-muted); }
  ol { display: flex; justify-content: center; gap: 24px; padding: 24px 0; margin: 24px 0 0; border-block-start: 1px solid var(--color-border); list-style: none; }
  li { display: flex; align-items: center; gap: 8px; font-size: 11px; font-weight: 500; }
  .step-number { display: grid; place-items: center; width: 22px; height: 22px; flex: none; border: 1px solid var(--color-border-strong); border-radius: 50%; font-size: 10px; font-variant-numeric: tabular-nums; }
  li[aria-current="step"] .step-number { background: var(--color-lit); border-color: var(--color-lit); color: var(--color-on-lit); }
  .upcoming, .completed { color: var(--color-muted); }
  @media (max-height: 600px) { .welcome { padding: 20px; } .welcome-card { padding-block: 24px; } .app-icon { width: 48px; height: 48px; margin-block-end: 16px; } ol { margin-block-start: 16px; padding-block: 16px; } }
  @media (max-width: 760px) { .welcome-card { width: min(100%, 400px); padding-inline: 24px; } ol { gap: 16px; } }
  @media (forced-colors: active) { .welcome-card { border-color: CanvasText; background: Canvas; box-shadow: none; } }
</style>
