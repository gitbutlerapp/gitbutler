<script lang="ts">
	import GitButler from '../images/gitbutler.svg';
	import { AuthService } from '$lib/auth/authService';
	import { getContext } from '@gitbutler/shared/context';
	import { env } from '$env/dynamic/public';

	const authService = getContext(AuthService);
	const token = $derived(authService.token);

	function logout() {
		authService.clearToken();
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/logout`;
	}

	function login() {
		console.log(env.PUBLIC_APP_HOST);
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/login`;
	}
</script>

<header>
	<a href="/" class="nav__left">
		<img src={GitButler} width="48" alt="github" />
		<h2>GitButler</h2>
	</a>
	<div>
		<a href="/downloads">Downloads</a>
		{#if $token}
			<!-- |
			<a href="/projects">Projects</a> -->
			|
			<a href="/organizations">Projects</a>
			|
			<a href="/repositories">Repositories</a>
			|
			<a href="/user">User</a>
		{/if}
	</div>
	<div class="nav__right">
		<button
			type="button"
			class="nav__right--button"
			onclick={() => {
				if ($token) {
					logout();
				} else {
					login();
				}
			}}
		>
			{$token ? 'Log Out' : 'Log In'}
		</button>
	</div>
</header>

<style>
	header {
		max-width: 64rem;
		width: 100%;
		margin: 0 auto;
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 16px;
	}

	a {
		color: unset;
	}

	.nav__left {
		display: flex;
		align-items: center;
		gap: 1rem;
	}

	.nav__right--button {
		/* Size */
		padding: 8px 16px;

		/* Style */
		--btn-text-clr: var(--clr-theme-pop-on-element);
		--btn-bg: var(--clr-theme-pop-element);

		font-size: 0.8rem;

		/* Btn Defaults */
		user-select: none;
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-m);
		cursor: pointer;
		color: var(--btn-text-clr);
		background: var(--btn-bg);
		transform-style: preserve-3d;
		backface-visibility: hidden;
		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast);
	}
</style>
