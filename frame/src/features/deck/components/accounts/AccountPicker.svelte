<script lang="ts">
  import { Select } from "bits-ui";
  import { ArrowDown01Icon } from "@hugeicons/core-free-icons";
  import type { AccountOption } from "$shared/ipc/bindings";
  import Icon from "$shared/ui/Icon";
  import Mark from "$shared/ui/Menu/Mark.svelte";
  import "$shared/ui/Menu/popover.css";
  import "$shared/ui/Menu/surface.css";
  import AccountAvatar from "./AccountAvatar.svelte";
  let { accounts, value, disabled = false, onchange }: { accounts: AccountOption[]; value: string; disabled?: boolean; onchange: (id: string) => void } = $props();
  const selected = $derived(accounts.find((account) => account.id === value));
</script>
<div class="account-picker">
  <Select.Root type="single" {value} {disabled} items={accounts.map((account) => ({ value: account.id, label: account.title }))} onValueChange={onchange}>
    <Select.Trigger class="account-trigger" aria-label={selected ? `アカウント：${selected.title}${selected.screen_name ? ` @${selected.screen_name}` : ""}` : "アカウント"}>
      {#if selected}<AccountAvatar url={selected.avatar} size={28} /><span class="identity"><strong>{selected.title}</strong>{#if selected.screen_name}<span>@{selected.screen_name}</span>{/if}</span>{:else}<span class="identity">アカウント</span>{/if}
      <Icon icon={ArrowDown01Icon} size={14} />
    </Select.Trigger>
    <Select.Portal>
      <Select.Content class="ui-menu account-options" data-menu-material="opaque" side="bottom" align="start" sideOffset={6} collisionPadding={12} onEscapeKeydown={(event) => event.stopPropagation()}>
        <Select.Viewport class="ui-menu-viewport account-viewport">
        {#each accounts as account (account.id)}
          <Select.Item class="ui-menu-item account-option" value={account.id} label={account.title}>
            {#snippet children({ selected })}
              <AccountAvatar url={account.avatar} size={30} /><span class="identity"><strong>{account.title}</strong>{#if account.screen_name}<span>@{account.screen_name}</span>{/if}</span><Mark on={selected} />
            {/snippet}
          </Select.Item>
        {/each}
        </Select.Viewport>
      </Select.Content>
    </Select.Portal>
  </Select.Root>
</div>
<style>
  .account-picker { width: 100%; min-width: 0; }
  .account-picker :global(.account-trigger), :global(.account-options .account-option) { display: flex; align-items: center; gap: 10px; width: 100%; box-sizing: border-box; padding: 8px 10px; border-radius: 8px; text-align: start; color: var(--color-text); }
  .account-picker :global(.ui-menu.account-options) { width: var(--bits-floating-anchor-width, auto); min-width: var(--bits-floating-anchor-width, 0); }
  .account-picker :global(.account-viewport) { max-height: 240px; }
  :global(.account-options .account-option) { background: transparent; min-height: 48px; }
  :global(.account-options .account-option[data-highlighted]) { background: var(--row-active); outline: none; }
  .identity { display: grid; flex: 1; min-width: 0; gap: 2px; }
  strong { font-size: 13px; font-weight: 550; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .identity > span { font-size: 11px; color: var(--color-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
