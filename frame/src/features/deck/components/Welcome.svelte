<script lang="ts">
  import icon from "../../../../../desktop/icons/128x128@2x.png";
</script>
<section class="welcome" aria-label="BDeckへようこそ">
  <img src={icon} width="96" height="96" alt="" />
  <h1>BDeckへようこそ</h1>
</section>
<style>
  .welcome { position: absolute; inset: 48px 0 0 56px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 24px; pointer-events: none; color: var(--color-text); }
  img { width: 96px; height: 96px; filter: drop-shadow(0 12px 20px rgb(0 0 0 / 0.12)); }
  h1 { margin: 0; font-size: clamp(22px, 3vw, 30px); font-weight: 500; letter-spacing: -0.04em; }
</style>
