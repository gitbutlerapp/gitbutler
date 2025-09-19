<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import AuthPageLayout from '$lib/components/auth/AuthPageLayout.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, EmailTextbox, Textbox, InfoMessage } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';

	let email = $state<string>();
	let password = $state<string>();

	let error = $state<string>();
	let errorCode = $state<string>();
	let confirmationSent = $state<boolean>(false);

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
			confirmationSent = false;
			console.error('Failed to resend confirmation email:', response.raw ?? response.errorMessage);
		} else {
			error = undefined;
			errorCode = undefined;
			confirmationSent = true;
			// Clear the confirmation message after 5 seconds
			setTimeout(() => {
				confirmationSent = false;
			}, 5000);
		}
	}
</script>

<svelte:head>
	<title>GitButler | Login</title>
</svelte:head>

<RedirectIfLoggedIn />

<AuthPageLayout
	title="Login"
	subtitle="to GitButler"
	oauthText="Or log in with"
	bottomLinkText="Don't have an account?"
	bottomLinkHref={routesService.signupPath()}
	bottomLinkLabel="Sign Up now"
>
	<form id="login-form" class="stack-v" onsubmit={handleSubmit}>
		<div class="auth-form__inputs">
			<EmailTextbox
				label="Email"
				placeholder=" "
				bind:value={email}
				autocomplete={false}
				autocorrect={false}
				spellcheck
			/>
			<Textbox bind:value={password} label="Password" type="password" />

			<div class="text-12 password-reset">
				<a href={routesService.resetPasswordPath()}>Forgot password?</a>
			</div>
		</div>

		{#if confirmationSent}
			<InfoMessage filled outlined={false} style="success" class="m-bottom-16">
				{#snippet content()}
					<p>Confirmation email sent! Please check your inbox.</p>
				{/snippet}
			</InfoMessage>
		{:else if error}
			<InfoMessage filled outlined={false} style="error" class="m-bottom-16">
				{#snippet content()}
					{#if errorCode === 'email_not_verified'}
						<p>
							Verify your email before logging in. Check your inbox or <button
								type="button"
								class="resend-confirm-btn"
								onclick={resendConfirmationEmail}
								disabled={!email}
							>
								resend the confirmation email</button
							>.
						</p>
					{:else}
						<p>{error}</p>
					{/if}
				{/snippet}
			</InfoMessage>
		{/if}

		<Button type="submit" style="pop" disabled={!isFormValid}>Log in</Button>
	</form>
</AuthPageLayout>

<style lang="postcss">
	.auth-form__inputs {
		display: flex;
		flex-direction: column;
		margin-bottom: 24px;
		gap: 14px;
	}

	.password-reset {
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

	.resend-confirm-btn {
		text-decoration: underline;
		text-decoration-style: dotted;
		cursor: pointer;
	}
</style>
