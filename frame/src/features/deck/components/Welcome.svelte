<script lang="ts">
  import icon from "../../../../../desktop/icons/128x128@2x.png";
</script>
<section class="welcome" aria-label="BDeckへようこそ">
  <div class="brand">
    <img src={icon} width="80" height="80" alt="" />
    <h1>BDeck</h1>
  </div>
</section>
<style>
  .welcome { position: absolute; inset: 48px 0 0 56px; display: grid; place-items: center; pointer-events: none; color: var(--color-text); }
  .brand { display: flex; flex-direction: column; align-items: center; gap: 32px; transform: translateY(-24px); }
  img { width: 80px; height: 80px; filter: drop-shadow(0 16px 24px rgb(0 0 0 / 0.16)); }
  h1 { margin: 0; padding-inline-start: 0.16em; font-family: -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif; font-size: clamp(40px, 6vw, 64px); font-weight: 300; line-height: 1; letter-spacing: 0.16em; }
</style>
