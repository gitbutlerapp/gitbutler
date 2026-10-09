<script lang="ts">
	// forgejo-fork: mirrors BitbucketUserLoginState.
	import ForgejoAccountBadge from "$components/forge/ForgejoAccountBadge.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { FORGEJO_USER_SERVICE } from "$lib/forge/forgejo/forgejoUserService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ForgeUserCard } from "@gitbutler/ui";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	import type { ForgejoAccountIdentifier } from "@gitbutler/but-sdk";

	type Props = {
		account: ForgejoAccountIdentifier;
	};

	const { account }: Props = $props();

	const forgejoUserService = inject(FORGEJO_USER_SERVICE);

	const [forget, forgetting] = forgejoUserService.forgetForgejoAccount;
	const fjUser = $derived(forgejoUserService.authenticatedUser(account));

	const isError = $derived(fjUser.result?.status === QueryStatus.rejected);
	const isLoading = $derived(fjUser.result?.status === QueryStatus.pending);
</script>

{#snippet card(
	username: string,
	avatarUrl: string | null,
	email: string | undefined,
	isError: boolean,
	isLoading: boolean,
)}
	<ForgeUserCard
		{username}
		{avatarUrl}
		{email}
		{isError}
		{isLoading}
		onForget={() => forget(account)}
		isForgetLoading={forgetting.current.isLoading}
	>
		{#snippet badge()}
			<ForgejoAccountBadge {account} />
		{/snippet}
	</ForgeUserCard>
{/snippet}

<ReduxResult result={fjUser.result}>
	{#snippet loading()}
		{@render card(account.username, null, undefined, false, true)}
	{/snippet}
	{#snippet error()}
		{@render card(account.username, null, undefined, true, false)}
	{/snippet}
	{#snippet children(user)}
		{@render card(
			user?.name ?? account.username,
			user?.avatarUrl ?? null,
			user?.email ?? undefined,
			isError,
			isLoading,
		)}
	{/snippet}
</ReduxResult>
