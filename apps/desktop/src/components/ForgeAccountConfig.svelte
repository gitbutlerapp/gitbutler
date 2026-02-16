<script lang="ts" generics="TAccount">
	import { useSettingsModal } from "$lib/settings/settingsModal.svelte";
	import { Button, CardGroup, Link, Select, SelectItem } from "@gitbutler/ui";
	import type { Component } from "svelte";

	type Props = {
		projectId: string;
		displayName: string;
		accounts: TAccount[];
		preferredAccount: TAccount | undefined;
		accountToString: (account: TAccount) => string;
		stringToAccount: (value: string) => TAccount | null;
		getUsername: (account: TAccount) => string;
		updatePreferredAccount: (projectId: string, account: TAccount) => void;
		AccountBadge: Component<{ account: TAccount; class?: string }>;
		docsUrl: string;
		requestType: "pull request" | "merge request";
	};

	const {
		projectId,
		displayName,
		accounts,
		preferredAccount,
		accountToString,
		stringToAccount,
		getUsername,
		updatePreferredAccount,
		AccountBadge,
		docsUrl,
		requestType,
	}: Props = $props();

	const { openGeneralSettings } = useSettingsModal();
	const hasAccounts = $derived(accounts.length > 0 && preferredAccount);

	function handleAccountChange(value: string) {
		const parsedAccount = stringToAccount(value);
		if (!parsedAccount) return;
		updatePreferredAccount(projectId, parsedAccount);
	}
</script>

<CardGroup.Item>
	{#snippet title()}
		{#if hasAccounts}
			Configure {displayName} integration
		{:else}
			Connect your {displayName} account
		{/if}
	{/snippet}

	{#snippet caption()}
		Enable {requestType} creation. Read more in the <Link href={docsUrl}>docs</Link>
	{/snippet}

	{#if !hasAccounts}
		<div class="flex">
			<Button onclick={() => openGeneralSettings("integrations")} style="pop" icon="link"
				>Set up in General Settings</Button
			>
		</div>
	{:else}
		{@const account = preferredAccount!}
		{@const accountStr = accountToString(account)}
		<Select
			label="{displayName} account for this project"
			value={accountStr}
			options={accounts.map((acc) => ({
				label: getUsername(acc),
				value: accountToString(acc),
			}))}
			onselect={handleAccountChange}
			disabled={accounts.length <= 1}
			wide
		>
			{#snippet itemSnippet({ item, highlighted })}
				{@const itemAccount = item.value && stringToAccount(item.value)}
				<SelectItem selected={item.value === accountStr} {highlighted}>
					{item.label}

					{#if itemAccount}
						<AccountBadge account={itemAccount} class="m-l-4" />
					{/if}
				</SelectItem>
			{/snippet}
		</Select>
	{/if}
</CardGroup.Item>
