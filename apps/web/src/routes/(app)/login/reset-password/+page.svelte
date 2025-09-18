<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, SectionCard } from '@gitbutler/ui';

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	let email = $state<string>();
	let error = $state<string>();
	let message = $state<string>();

	async function handleSubmit() {
		if (!email) {
			error = 'Email is required';
			return;
		}

		const response = await loginService.resetPassword(email);
		if (response.type === 'error') {
			error = response.errorMessage;
			console.error('Reset password failed:', response.raw ?? response.errorMessage);
		} else {
			error = undefined;
			message = response.data.message;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Login</title>
</svelte:head>

<RedirectIfLoggedIn />

<div class="main-links">
	<a href={routesService.homePath()} class="logo" aria-label="main nav" title="Home">
		<svg width="23" height="22" viewBox="0 0 23 22" fill="none" xmlns="http://www.w3.org/2000/svg">
			<path d="M0 22V0L11.4819 9.63333L23 0V22L11.4819 12.4L0 22Z" fill="var(--clr-text-1)" />
		</svg>
	</a>
</div>

<form onsubmit={handleSubmit} class="signin-form">
	<div class="signup-form__content">
		<SectionCard>
			<div class="field">
				<label for="email">Email</label>
				<input id="email" type="email" bind:value={email} required />
			</div>
		</SectionCard>

		<Button type="submit">Send password reset mail</Button>

		<div class="login-link">
			You rather just create a new account?
			<a href={routesService.signupPath()}>Sound easier, I guess</a>
		</div>

		{#if error}
			<div class="error-message">{error}</div>
		{/if}

		{#if message}
			<div class="message">{message}</div>
		{/if}
	</div>
</form>

<style lang="postcss">
	.main-links {
		display: flex;
		align-items: center;
		margin-bottom: 16px;
		overflow: hidden;
		gap: 16px;
	}

	.logo {
		display: flex;
	}

	.signup-form__content {
		display: flex;
		flex-direction: column;
		max-width: 400px;
		margin: auto;
		gap: 16px;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;

		label {
			color: var(--clr-scale-ntrl-30);
			font-size: 14px;
		}

		input {
			padding: 8px 12px;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
			background-color: var(--clr-bg-1);
			color: var(--clr-scale-ntrl-0);
			font-size: 14px;

			&:read-only {
				cursor: not-allowed;
				opacity: 0.7;
			}

			&:not(:read-only) {
				&:focus {
					border-color: var(--clr-scale-pop-70);
					outline: none;
				}
			}
		}
	}

	.error-message {
		padding: 8px;
		border: 1px solid var(--clr-scale-err-60);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-err-bg-muted);
		color: var(--clr-scale-err-10);
		font-size: 14px;
	}

	.message {
		padding: 8px;
		border: 1px solid var(--clr-scale-succ-60);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-succ-bg-muted);
		color: var(--clr-scale-err-10);
		font-size: 14px;
	}

	.login-link {
		display: flex;
		justify-content: center;
		gap: 4px;
		font-size: 14px;

		a {
			text-decoration: underline;
		}
	}
</style>
