<script lang="ts">
	import SectionCard from '$lib/components/SectionCard.svelte';
	import { getGitHubUserServiceStore } from '$lib/gitHost/github/githubUserService';
	import RadioButton from '$lib/shared/RadioButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { GitHubLoginListItem } from '$lib/stores/user';

	interface Props {
		login: GitHubLoginListItem;
		disabled: boolean;
		radioButtonName: string;
	}

	const { login, disabled, radioButtonName }: Props = $props();

	const githubUserService = getGitHubUserServiceStore();

	$effect(() => {
		if ($githubUserService) $githubUserService?.getUserInfo(login.username);
	});

	const userMap = $derived($githubUserService?.userMap);
	const userData = $derived($userMap?.[login.username]);

	function forgetProfile() {
		// TODO: Implement
	}
</script>

<SectionCard orientation="row" roundedTop={false} roundedBottom={false}>
	<svelte:fragment slot="iconSide">
		{#if userData}
			<img class="profile-pic" src={userData.avatar_url} alt="" referrerpolicy="no-referrer" />
		{/if}
	</svelte:fragment>
	<svelte:fragment slot="title">{login.username}</svelte:fragment>
	<svelte:fragment slot="caption">
		<span>{userData?.name}</span>
	</svelte:fragment>
	<svelte:fragment slot="actions">
		<div class="actions-container">
			<Button style="ghost" outline {disabled} icon="bin-small" onclick={forgetProfile}
				>Forget</Button
			>
			<RadioButton
				name={radioButtonName}
				id="github-login-{login.username}"
				value={login.username}
				checked={login.selected}
			/>
		</div>
	</svelte:fragment>
</SectionCard>

<style>
	.profile-pic {
		width: 28px;
		height: 28px;
		border-radius: 50%;
	}

	.actions-container {
		display: flex;
		align-items: center;
		gap: 16px;
	}
</style>
