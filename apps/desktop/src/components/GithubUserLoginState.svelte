<script lang="ts">
	import GitHubAccountBadge from "$components/GitHubAccountBadge.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { GITHUB_USER_SERVICE } from "$lib/forge/github/githubUserService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ForgeUserCard } from "@gitbutler/ui";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	import type { ButGitHubToken } from "@gitbutler/core/api";

	type Props = {
		account: ButGitHubToken.GithubAccountIdentifier;
	};

	const { account }: Props = $props();

	const githubUserService = inject(GITHUB_USER_SERVICE);

	const [forget, forgetting] = githubUserService.forgetGitHubUsername;
	const ghUser = $derived(githubUserService.authenticatedUser(account));

	const isError = $derived(ghUser.result?.status === QueryStatus.rejected);
	const isLoading = $derived(ghUser.result?.status === QueryStatus.pending);
</script>

<ReduxResult result={ghUser.result}>
	{#snippet loading()}
		<ForgeUserCard
			username={account.info.username}
			avatarUrl={null}
			isError={false}
			isLoading={true}
			onForget={() => forget(account)}
			isForgetLoading={forgetting.current.isLoading}
		>
			{#snippet badge()}
				<GitHubAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
	{#snippet error()}
		<ForgeUserCard
			username={account.info.username}
			avatarUrl={null}
			isError={true}
			isLoading={false}
			onForget={() => forget(account)}
			isForgetLoading={forgetting.current.isLoading}
		>
			{#snippet badge()}
				<GitHubAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
	{#snippet children(user)}
		<ForgeUserCard
			username={account.info.username}
			avatarUrl={user?.avatarUrl ?? null}
			email={user?.email}
			{isError}
			{isLoading}
			onForget={() => forget(account)}
			isForgetLoading={forgetting.current.isLoading}
		>
			{#snippet badge()}
				<GitHubAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
</ReduxResult>
