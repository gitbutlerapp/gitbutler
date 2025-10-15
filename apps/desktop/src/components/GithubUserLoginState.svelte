<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import githubLogoSvg from '$lib/assets/unsized-logos/github.svg?raw';
	import { GITHUB_USER_SERVICE } from '$lib/forge/github/githubUserService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Avatar, Button, SectionCard } from '@gitbutler/ui';

	type Props = {
		username: string;
		disabled?: boolean;
	};

	const { username, disabled }: Props = $props();

	const githubUserService = inject(GITHUB_USER_SERVICE);

	const [forget, forgetting] = githubUserService.forgetGitHubUsername;
	const ghUser = $derived(githubUserService.authenticatedUser(username));
</script>

<ReduxResult result={ghUser.result}>
	{#snippet loading()}
		<SectionCard orientation="row">
			{#snippet iconSide()}
				<div class="icon-wrapper__logo">
					{@html githubLogoSvg}
				</div>
			{/snippet}
			{#snippet title()}
				<p>{username}</p>
			{/snippet}
		</SectionCard>
	{/snippet}

	{#snippet children(user)}
		<SectionCard orientation="row">
			{#snippet iconSide()}
				{#if user?.avatarUrl}
					<Avatar size="large" tooltip={username} srcUrl={user?.avatarUrl} />
				{:else}
					<div class="icon-wrapper__logo">
						{@html githubLogoSvg}
					</div>
				{/if}
			{/snippet}
			{#snippet title()}
				<p>{username}</p>
			{/snippet}

			<Button
				kind="outline"
				icon="bin-small"
				disabled={disabled || user === null}
				onclick={() => forget(username)}
				loading={forgetting.current.isLoading}>Forget</Button
			>
		</SectionCard>
	{/snippet}
</ReduxResult>

<style>
	.icon-wrapper__logo {
		width: 28px;
		height: 28px;
	}
</style>
