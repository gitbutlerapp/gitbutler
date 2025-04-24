<script lang="ts">
	import { AuthService } from '$lib/auth/authService.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';

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
	<div class="dashboard">
		<h1>Welcome to GitButler Cloud</h1>
	</div>
{/if}

<style>
	.dashboard h1 {
		font-size: 2.5rem;
	}
</style>
