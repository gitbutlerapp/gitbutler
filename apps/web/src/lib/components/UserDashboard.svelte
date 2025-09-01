<script lang="ts">
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';

	const authService = inject(AUTH_SERVICE);
	const userService = inject(USER_SERVICE);

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
