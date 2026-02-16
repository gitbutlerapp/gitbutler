<script lang="ts">
	import GitLabAccountBadge from "$components/GitLabAccountBadge.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { GITLAB_USER_SERVICE } from "$lib/forge/gitlab/gitlabUserService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ForgeUserCard } from "@gitbutler/ui";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	import type { ButGitLabToken } from "@gitbutler/core/api";

	type Props = {
		account: ButGitLabToken.GitlabAccountIdentifier;
	};

	const { account }: Props = $props();

	const gitlabUserService = inject(GITLAB_USER_SERVICE);

	const [forget, forgetting] = gitlabUserService.forgetGitLabAccount;
	const glUser = $derived(gitlabUserService.authenticatedUser(account));

	const isError = $derived(glUser.result?.status === QueryStatus.rejected);
	const isLoading = $derived(glUser.result?.status === QueryStatus.pending);

	const username = $derived(
		account.type === "patUsername" ? account.info.username : account.info.username,
	);
</script>

<ReduxResult result={glUser.result}>
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
				<GitLabAccountBadge {account} />
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
				<GitLabAccountBadge {account} />
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
				<GitLabAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
</ReduxResult>
