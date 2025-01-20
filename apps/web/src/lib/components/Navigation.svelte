<script lang="ts">
	import { AuthService } from '$lib/auth/authService';
	import Breadcrumbs from '$lib/components/breadcrumbs/Breadcrumbs.svelte';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { env } from '$env/dynamic/public';

	const routes = getContext(WebRoutesService);

	const authService = getContext(AuthService);
	const token = $derived(authService.token);

	const userService = getContext(UserService);
	const user = $derived(userService.user);

	function login() {
		window.location.href = `${env.PUBLIC_APP_HOST}/cloud/login?callback=${window.location.href}`;
	}
	function logout() {
		authService.clearToken();
		window.location.href = `${env.PUBLIC_APP_HOST}/cloud/logout`;
	}
</script>

<div class="navigation">
	<div class="main-links">
		<div class="link">
			<a href="/" class="main-nav" aria-label="main nav" title="Home">
				<svg
					xmlns="http://www.w3.org/2000/svg"
					width="23"
					height="24"
					viewBox="0 0 23 24"
					fill="none"
				>
					<path d="M0 24V0L11.4819 10.5091L23 0V24L11.4819 13.5273L0 24Z" fill="black" />
				</svg>
			</a>
		</div>
		<div class="link">
			<Breadcrumbs />
		</div>
	</div>

	<div class="account-links">
		{#if $token}
			<div class="link">
				<a class="nav-link nav-button" href="/organizations" aria-label="organizations">
					Organizations
				</a>
			</div>
			<div class="link">
				<a class="nav-link nav-button" href={routes.projectsPath()} aria-label="projects"
					>Projects</a
				>
			</div>
		{/if}
		<div class="link">
			<a class="nav-link nav-button" href="/downloads" aria-label="downloads" title="Downloads">
				Downloads
			</a>
		</div>

		{#if $user}
			<div>
				<a href="/profile" class="nav-link nav-button" aria-label="profile">
					<img class="user__id--img" alt="User Avatar" width="48" src={$user.picture} />
				</a>
			</div>
		{/if}

		<div>
			<button
				type="button"
				class="nav-link nav-button"
				onclick={() => {
					if ($token) {
						logout();
					} else {
						login();
					}
				}}
			>
				{#if $token}
					Log Out
				{:else}
					Log In
				{/if}
			</button>
		</div>
	</div>
</div>

<style>
	.navigation {
		display: flex;
		justify-content: space-between;
		width: 100%;
		height: 64px;
		padding: 0 16px;
	}

	.user__id--img {
		width: 28px;
		height: 28px;
		border-radius: 0.5rem;
	}

	.account-links {
		display: flex;
		flex-direction: row;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
	}

	.main-links {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 24px;
	}

	.nav-button {
		display: flex;
		border-radius: var(--radius-s);
		white-space: nowrap;
	}
</style>
