<script lang="ts">
	import walkininSvg from '$lib/assets/splash-illustrations/walkin.svg?raw';
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import OAuthButtons from '$lib/components/login/OAuthButtons.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, EmailTextbox, Textbox } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';

	let email = $state<string>();
	let password = $state<string>();

	let error = $state<string>();
	let errorCode = $state<string>();

	const isFormValid = $derived(!!email && !!password);

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

<div class="login-page">
	<div class="login-form__container">
		<form onsubmit={handleSubmit} class="login-form">
			<h1 class="text-serif-42 m-bottom-40">
				<i>Login</i>
				to GitButler
			</h1>

			<div class="login-form__inputs">
				<EmailTextbox
					label="Email"
					placeholder=" "
					bind:value={email}
					autocomplete={false}
					autocorrect={false}
					spellcheck
				/>
				<Textbox bind:value={password} label="Password" type="password" />

				<div class="text-12 login-form__password-reset">
					<a href={routesService.resetPasswordPath()}>Forgot password?</a>
				</div>
			</div>

			<Button type="submit" style="pop" disabled={!isFormValid}>Log in</Button>

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

			<div class="login-form__social">
				<div class="login-form__social-title">
					<span class="text-12"> Or log in with </span>
				</div>

				<OAuthButtons />
			</div>

			<div class="text-12 signup-link">
				<p>Don't have an account? <a href={routesService.signupPath()}>Sign Up now</a></p>
			</div>
		</form>

		<div class="login-form__illustration">
			{@html walkininSvg}
		</div>
	</div>
</div>

<style lang="postcss">
	.login-page {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.login-form__container {
		display: flex;
		width: 100%;
		max-width: 1000px;
		overflow: hidden;
		border-radius: var(--radius-xl);
	}

	.login-form {
		display: flex;
		flex: 4;
		flex-direction: column;
		width: 100%;
		padding: 50px 80px 30px;
		/* gap: 40px; */
		background-color: var(--clr-bg-1);
	}

	.login-form__inputs {
		display: flex;
		flex-direction: column;
		margin-bottom: 24px;
		gap: 14px;
	}

	.login-form__password-reset {
		display: flex;
		justify-content: flex-end;
		color: var(--clr-text-2);

		a {
			text-decoration: none;

			&:hover {
				color: var(--clr-text-1);
				text-decoration: underline;
			}
		}
	}

	.login-form__social {
		display: flex;
		flex-direction: column;
		margin-top: 24px;
	}

	.login-form__social-title {
		display: flex;
		justify-content: center;
		margin-bottom: 16px;
		color: var(--clr-text-2);

		span {
			margin: 0 12px;
		}

		&::before,
		&::after {
			flex: 1;
			margin: auto 0;
			border-bottom: 1px solid var(--clr-border-2);
			content: '';
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

	.signup-link {
		display: flex;
		align-items: center;
		justify-content: center;
		margin-top: 40px;
		gap: 4px;
		color: var(--clr-text-2);

		a {
			text-decoration: underline;

			&:hover {
				color: var(--clr-text-1);
			}
		}
	}

	.login-form__illustration {
		display: flex;
		flex: 4;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 32px;
		background-color: var(--clr-illustration-bg);

		:global(svg) {
			max-width: 400px;
		}
	}
</style>
