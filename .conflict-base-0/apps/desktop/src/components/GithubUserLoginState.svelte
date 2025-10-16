<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import githubLogoSvg from '$lib/assets/unsized-logos/github.svg?raw';
	import { GITHUB_USER_SERVICE } from '$lib/forge/github/githubUserService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Avatar, Button, Icon, SectionCard } from '@gitbutler/ui';

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
	{#snippet error()}
		<SectionCard orientation="row">
			{#snippet iconSide()}
				<div class="icon-wrapper">
					<div class="icon-wrapper__tick">
						<Icon name="error" color="error" size={18} />
					</div>
					<div class="icon-wrapper__logo">
						{@html githubLogoSvg}
					</div>
				</div>
			{/snippet}
			{#snippet title()}
				<p>{username}</p>
			{/snippet}
			{#snippet caption()}
				<p>Error loading GitHub user</p>
			{/snippet}

			<Button
				kind="outline"
				icon="bin-small"
				{disabled}
				onclick={() => forget(username)}
				loading={forgetting.current.isLoading}>Forget</Button
			>
		</SectionCard>
	{/snippet}

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
				<div class="icon-wrapper">
					{#if !user}
						<div class="icon-wrapper__tick">
							<Icon name="error" color="error" size={18} />
						</div>
					{/if}
					{#if user?.avatarUrl}
						<Avatar size="large" tooltip={username} srcUrl={user?.avatarUrl} />
					{:else}
						<div class="icon-wrapper__logo">
							{@html githubLogoSvg}
						</div>
					{/if}
				</div>
			{/snippet}
			{#snippet title()}
				<p>{username}</p>
			{/snippet}
			{#snippet caption()}
				{#if !user}
					<p>GitHub user not found</p>
				{/if}
			{/snippet}

			<Button
				kind="outline"
				icon="bin-small"
				{disabled}
				onclick={() => forget(username)}
				loading={forgetting.current.isLoading}>Forget</Button
			>
		</SectionCard>
	{/snippet}
</ReduxResult>

<style>
	.icon-wrapper {
		position: relative;
		align-self: flex-start;
		height: fit-content;
	}

	.icon-wrapper__tick {
		display: flex;
		position: absolute;
		right: -4px;
		bottom: -4px;
		align-items: center;
		justify-content: center;
		border-radius: 50px;
		background-color: var(--clr-scale-ntrl-100);
	}

	.icon-wrapper__logo {
		width: 28px;
		height: 28px;
	}
</style>
