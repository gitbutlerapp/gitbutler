<script lang="ts">
	import '$lib/styles/global.css';
	import { AuthService } from '$lib/auth/authService';
	import Navigation from '$lib/components/Navigation.svelte';
	import { UserService } from '$lib/user/userService';
	import { HttpClient } from '@gitbutler/shared/httpClient';
	import { setContext, type Snippet } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { env } from '$env/dynamic/public';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();

	const authService = new AuthService();
	setContext(AuthService, authService);

	const httpClient = new HttpClient(window.fetch, env.PUBLIC_APP_HOST, authService.token);
	setContext(HttpClient, httpClient);

	const userService = new UserService(httpClient);
	setContext(UserService, userService);

	$effect(() => {
		if ($page.url.searchParams.get('gb_access_token')) {
			const token = $page.url.searchParams.get('gb_access_token');
			if (token && token.length > 0) {
				$page.data.authService.setToken(token);

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

	<footer>
		<p>GitButler</p>
	</footer>
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

	footer {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		padding: 12px;
	}

	@media (min-width: 480px) {
		footer {
			padding: 12px 0;
		}
	}
</style>
