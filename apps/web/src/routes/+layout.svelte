<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { env } from '$env/dynamic/public';

	// onMount, check for page params
	let token: string | null = null;
	onMount(() => {
		// if searchparams has token, save it to localstorage
		if ($page.url.searchParams.get('gb_access_token')) {
			let token = $page.url.searchParams.get('gb_access_token');
			if (token && token.length > 0) {
				localStorage.setItem('gb_access_token', token);
				// redirect to remove search params
				window.location.href = '/';
			}
		}
		if (localStorage.has('gb_access_token')) {
			token = localStorage.getItem('gb_access_token');
		}
	});

	function logout() {
		localStorage.removeItem('gb_access_token');
		token = null;
		window.location.href = env.PUBLIC_APP_HOST + 'cloud/logout';
	}
</script>

<div class="app">
	<header>
		<h2>GitButler</h2>
		<div>
			<a href="/user">User</a>
			|
			<a href="/downloads">Downloads</a>
		</div>
		<div class="login">
			{#if token}
				<p><button on:click={logout}>Log Out</button></p>
			{:else}
				<p><a href={`${env.PUBLIC_APP_HOST}cloud/login`}>Log In</a></p>
			{/if}
		</div>
	</header>

	<main>
		<slot />
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
		max-width: 64rem;
		margin: 0 auto;
		box-sizing: border-box;
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

	header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 1px 12px;
		border-bottom: 1px solid #ccc;
	}
</style>
