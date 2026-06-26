<script lang="ts">
	import BitbucketAccountBadge from "$components/forge/BitbucketAccountBadge.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { BITBUCKET_USER_SERVICE } from "$lib/forge/bitbucket/bitbucketUserService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ForgeUserCard } from "@gitbutler/ui";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	import type { BitbucketAccountIdentifier } from "@gitbutler/but-sdk";

	type Props = {
		account: BitbucketAccountIdentifier;
	};

	const { account }: Props = $props();

	const bitbucketUserService = inject(BITBUCKET_USER_SERVICE);

	const [forget, forgetting] = bitbucketUserService.forgetBitbucketAccount;
	const bbUser = $derived(bitbucketUserService.authenticatedUser(account));

	const isError = $derived(bbUser.result?.status === QueryStatus.rejected);
	const isLoading = $derived(bbUser.result?.status === QueryStatus.pending);

	const username = $derived(account.info.email);
</script>

<ReduxResult result={bbUser.result}>
	{#snippet loading()}
		<ForgeUserCard
			{username}
			avatarUrl={null}
			isError={false}
			isLoading={true}
			onForget={() => forget(account)}
			isForgetLoading={forgetting.current.isLoading}
		>
			{#snippet badge()}
				<BitbucketAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
	{#snippet error()}
		<ForgeUserCard
			{username}
			avatarUrl={null}
			isError={true}
			isLoading={false}
			onForget={() => forget(account)}
			isForgetLoading={forgetting.current.isLoading}
		>
			{#snippet badge()}
				<BitbucketAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
	{#snippet children(user)}
		<ForgeUserCard
			username={user?.name ?? username}
			avatarUrl={user?.avatarUrl ?? null}
			email={user?.email ?? username}
			{isError}
			{isLoading}
			onForget={() => forget(account)}
			isForgetLoading={forgetting.current.isLoading}
		>
			{#snippet badge()}
				<BitbucketAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
</ReduxResult>
