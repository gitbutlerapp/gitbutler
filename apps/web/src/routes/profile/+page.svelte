<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import { featureShowOrganizations, featureShowProjectPage } from '$lib/featureFlags';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	const authService = getContext(AuthService);
	const userService = getContext(UserService);

	const user = $derived(userService.user);
	const token = $derived(authService.tokenReadable);
</script>

<svelte:head>
	<title>GitButler | User</title>
</svelte:head>

{#if !$token}
	<p>Unauthorized</p>
{:else if !$user?.id}
	<p>Loading...</p>
{:else}
	<div class="profile">
		<h1>Your Profile</h1>
		<div><b>Login</b>: {$user?.login}</div>
		<div><b>Email</b>: {$user?.email}</div>
		<div><b>Joined</b>: {$user?.created_at}</div>
		<div><b>Supporter</b>: {$user?.supporter}</div>
	</div>
{/if}

<div class="experimental-settings">
	<h1>Experimental settings</h1>
	<SectionCard labelFor="showOrganizations" orientation="row">
		{#snippet title()}Organizations{/snippet}
		{#snippet caption()}
			Organizations are a way of linking together projects.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="showOrganizations"
				checked={$featureShowOrganizations}
				onclick={() => ($featureShowOrganizations = !$featureShowOrganizations)}
			/>
		{/snippet}
	</SectionCard>
	<SectionCard labelFor="showProjectPage" orientation="row">
		{#snippet title()}Project Page{/snippet}
		{#snippet caption()}
			The project page provides an overview of the project.
		{/snippet}
		{#snippet actions()}
			<Toggle
				id="showProjectPage"
				checked={$featureShowProjectPage}
				onclick={() => ($featureShowProjectPage = !$featureShowProjectPage)}
			/>
		{/snippet}
	</SectionCard>
</div>

<style>
	h1 {
		font-size: 1.5rem;
		margin-bottom: 10px;
	}
	.profile,
	.experimental-settings {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 32px;
	}
</style>
