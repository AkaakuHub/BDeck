<script lang="ts">
  import { deck, DEFAULT_ROW_RATIO, MIN_ROW_RATIO, MAX_ROW_RATIO } from "$domain/deck";
  import ResettableRange from "../../controls/ResettableRange.svelte";
  import SettingsGroup from "$shared/ui/SettingsGroup";
  import SettingsRow from "$shared/ui/SettingsRow";
  import Switch from "$shared/ui/Switch";
  import Select from "$shared/ui/Select";
  import SegmentedControl from "$shared/ui/SegmentedControl";
  import SettingsTransfer from "./SettingsTransfer.svelte";
  import UpdateSection from "./UpdateSection.svelte";
  const active = $derived(deck.active());
</script>

<SettingsGroup title="投稿の表示">
  <SettingsRow title="文字サイズ">
    <Select label="投稿の文字サイズ" labelHidden value={String(deck.appearance().font_size)} options={[12, 13, 14, 15, 16, 17, 18].map((size) => ({ value: String(size), label: `${size}px` }))} onchange={(value) => void deck.setAppearance({ font_size: Number(value) })} />
  </SettingsRow>
  <SettingsRow title="投稿の余白">
    <SegmentedControl label="投稿の余白" size="compact" value={deck.appearance().density} options={[{ value: "comfortable", label: "標準" }, { value: "compact", label: "狭く" }]} onchange={(value) => { if (value === "comfortable" || value === "compact") void deck.setAppearance({ density: value }); }} />
  </SettingsRow>
  <SettingsRow title="アバターの形">
    <SegmentedControl label="アバターの形" size="compact" value={deck.appearance().avatars} options={[{ value: "circle", label: "円形" }, { value: "square", label: "四角" }]} onchange={(value) => { if (value === "circle" || value === "square") void deck.setAppearance({ avatars: value }); }} />
  </SettingsRow>
  <SettingsRow title="画像と動画">
    <Select label="画像と動画の表示" labelHidden value={deck.appearance().media} options={[{ value: "native", label: "Xのサイズを使用" }, { value: "compact", label: "小さく表示" }, { value: "hidden", label: "非表示" }]} onchange={(value) => { if (value === "native" || value === "compact" || value === "hidden") void deck.setAppearance({ media: value }); }} />
  </SettingsRow>
</SettingsGroup>
<SettingsGroup title="カラムの表示">
  <SettingsRow title="スクロールバー">
    <Select label="スクロールバー" labelHidden value={deck.appearance().scrollbars} options={[{ value: "system", label: "標準" }, { value: "thin", label: "細く" }, { value: "hidden", label: "非表示" }]} onchange={(value) => { if (value === "system" || value === "thin" || value === "hidden") void deck.setAppearance({ scrollbars: value }); }} />
  </SettingsRow>
  <SettingsRow title="広告を非表示">
    <Switch label="広告を非表示" checked={deck.hideAdvertisements()} onchange={(value) => void deck.setHideAdvertisements(value)} />
  </SettingsRow>
  <SettingsRow title="Xのナビゲーションを表示">
    <Switch label="Xのナビゲーションを表示" checked={!deck.appearance().hide_navigation} onchange={(value) => void deck.setAppearance({ hide_navigation: !value })} />
  </SettingsRow>
</SettingsGroup>


{#if active?.layout === "two_rows"}
<SettingsGroup title="カラムの高さ">
  <SettingsRow title="上段の割合">
    <div class="row-ratio"><ResettableRange label="上段の割合" labelHidden value={active.row_ratio} defaultValue={DEFAULT_ROW_RATIO} min={MIN_ROW_RATIO} max={MAX_ROW_RATIO} unit="%" onchange={(value) => deck.setRowRatio(value)} /></div>
  </SettingsRow>
</SettingsGroup>
{/if}
<SettingsTransfer />
<UpdateSection />
<style>.row-ratio { inline-size: 240px; max-inline-size: 100%; }</style>
