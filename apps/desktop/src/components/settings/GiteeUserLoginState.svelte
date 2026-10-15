<script lang="ts">
	import GiteeAccountBadge from "$components/forge/GiteeAccountBadge.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { GITEE_USER_SERVICE } from "$lib/forge/gitee/giteeUserService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ForgeUserCard } from "@gitbutler/ui-svelte";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	import type { GiteeAccountIdentifier } from "@gitbutler/but-sdk";

	type Props = {
		account: GiteeAccountIdentifier;
	};

	const { account }: Props = $props();

	const giteeUserService = inject(GITEE_USER_SERVICE);

	const [forget, forgetting] = giteeUserService.forgetGiteeAccount;
	const giteeUser = $derived(giteeUserService.authenticatedUser(account));

	const isError = $derived(giteeUser.result?.status === QueryStatus.rejected);
	const isLoading = $derived(giteeUser.result?.status === QueryStatus.pending);

	const username = $derived(account.info.username);
</script>

<ReduxResult result={giteeUser.result}>
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
				<GiteeAccountBadge {account} />
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
				<GiteeAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
	{#snippet children(user)}
		<ForgeUserCard
			{username}
			avatarUrl={user?.avatarUrl ?? null}
			email={user?.email}
			{isError}
			{isLoading}
			onForget={() => forget(account)}
			isForgetLoading={forgetting.current.isLoading}
		>
			{#snippet badge()}
				<GiteeAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
</ReduxResult>
