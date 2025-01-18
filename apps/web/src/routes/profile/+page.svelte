<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import { cleanBreadcrumbs } from '$lib/components/breadcrumbs/breadcrumbsContext.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';

	const authService = getContext(AuthService);
	const userService = getContext(UserService);

	const user = $derived(userService.user);
	const token = $derived(authService.token);

	$effect(cleanBreadcrumbs);
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

<style>
	h1 {
		font-size: 1.5rem;
		margin-bottom: 10px;
	}
	.profile {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: 32px;
	}
</style>
