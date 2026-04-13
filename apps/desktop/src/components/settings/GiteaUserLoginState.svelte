<script lang="ts">
	import GiteaAccountBadge from "$components/forge/GiteaAccountBadge.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { GITEA_USER_SERVICE } from "$lib/forge/gitea/giteaUserService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ForgeUserCard } from "@gitbutler/ui";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	// Assuming a similar type structure for Gitea accounts
	type Props = {
		account: any; 
	};

	const { account }: Props = $props();

	const giteaUserService = inject(GITEA_USER_SERVICE);

	const [forget, forgetting] = giteaUserService.forgetGiteaUsername;
	const giteaUser = $derived(giteaUserService.authenticatedUser(account));

	const isError = $derived(giteaUser.result?.status === QueryStatus.rejected);
	const isLoading = $derived(giteaUser.result?.status === QueryStatus.pending);
</script>

<ReduxResult result={giteaUser.result}>
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
				<GiteaAccountBadge {account} />
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
				<GiteaAccountBadge {account} />
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
				<GiteaAccountBadge {account} />
			{/snippet}
		</ForgeUserCard>
	{/snippet}
</ReduxResult>
