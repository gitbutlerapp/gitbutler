<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import GitHubButton from '$lib/components/login/GitHubButton.svelte';
	import GoogleButton from '$lib/components/login/GoogleButton.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, SectionCard } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';

	let email = $state<string>();
	let password = $state<string>();

	let error = $state<string>();
	let errorCode = $state<string>();

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);
	const authService = inject(AUTH_SERVICE);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		if (!email || !password) {
			console.error('Email and password are required');
			return;
		}

		const response = await loginService.loginWithEmail(email, password);

		if (response.type === 'error') {
			error = response.errorMessage;
			errorCode = response.errorCode;
			console.error('Login failed:', response.raw ?? response.errorMessage);
		} else {
			const token = response.data;
			authService.setToken(token);
			window.location.href = `${env.PUBLIC_APP_HOST}successful_login?access_token=${token}`;
		}
	}

	async function resendConfirmationEmail() {
		if (!email) {
			error = 'Please enter your email to resend the confirmation email.';
			return;
		}
		const response = await loginService.resendConfirmationEmail(email);
		if (response.type === 'error') {
			error = response.errorMessage;
			errorCode = response.errorCode;
			console.error('Failed to resend confirmation email:', response.raw ?? response.errorMessage);
		} else {
			error = 'Confirmation email resent. Please check your inbox.';
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

<form onsubmit={handleSubmit} class="login-form">
	<div class="login-form__content">
		<SectionCard>
			<div class="field">
				<label for="email">Email</label>
				<input id="email" type="email" bind:value={email} required />
			</div>
			<div class="field">
				<label for="password">Password</label>
				<input id="password" type="password" bind:value={password} required />
			</div>
		</SectionCard>

		<div class="reset-password-link">
			Or did you forget your password?
			<br />
			Wow. That sounds irresponsible.
			<a href={routesService.resetPasswordPath()}>I mean... it's fine. Reset it</a>
		</div>

		<Button type="submit">Log in</Button>

		<div class="signup-link">
			Don't have an account?
			<a href={routesService.signupPath()}>Sign up</a>
		</div>

		{#if error}
			<div class="error-message">
				<span>
					{error}
				</span>
			</div>
			{#if errorCode === 'email_not_verified'}
				<span>
					Please verify your email address before logging in. Check your inbox for a verification
					email or
				</span>
				<Button
					type="button"
					onclick={resendConfirmationEmail}
					disabled={!email}
					tooltip={!email
						? 'Please enter your email in the field above to resend the confirmation email.'
						: 'Resend confirmation email'}
				>
					resend the confirmation email</Button
				>
			{/if}
		{/if}

		<SectionCard>
			<GitHubButton />
			<GoogleButton />
		</SectionCard>
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

	.login-form__content {
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
		display: flex;
		flex-direction: column;
		padding: 8px;
		gap: 8px;
		border: 1px solid var(--clr-scale-err-60);
		border-radius: var(--radius-m);

		background-color: var(--clr-theme-err-bg-muted);
		color: var(--clr-scale-err-10);
		font-size: 14px;
	}

	.signup-link,
	.reset-password-link {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;

		gap: 4px;
		font-size: 14px;

		a {
			text-decoration: underline;
		}
	}
</style>
