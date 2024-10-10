<script lang="ts">
	import '$lib/styles/global.css';
	import { AuthService } from '$lib/auth/authService';
	import Navigation from '$lib/components/Navigation.svelte';
	import { UserService } from '$lib/user/userService';
	import { setContext, type Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	setContext(AuthService, $page.data.authService);
	setContext(UserService, $page.data.userService);

	$effect(() => {
		if ($page.url.searchParams.get('gb_access_token')) {
			const token = $page.url.searchParams.get('gb_access_token');
			if (token && token.length > 0) {
				$page.data.authService.setToken(token);
				localStorage.setItem('gb_access_token', token);

				$page.url.searchParams.delete('gb_access_token');
				goto(`?${$page.url.searchParams.toString()}`);
			}
		}
	});
</script>

<div class="app">
	<Navigation />

	<main>
		{@render children()}
	</main>
</div>

<style>
	.app {
		display: flex;
		flex-direction: column;
		min-height: 100vh;
	}

	main {
		flex: 1;
		display: flex;
		flex-direction: column;
		padding: 1rem;
		width: 100%;
		max-width: 84rem;
		margin: 0 auto;
	}
</style>
