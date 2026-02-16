<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import OAuthButtons from '$lib/components/auth/OAuthButtons.svelte';
	import FullscreenIllustrationCard from '$lib/components/service/FullscreenIllustrationCard.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, EmailTextbox, Textbox, InfoMessage } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';

	let email = $state<string>();
	let password = $state<string>();

	let emailTextbox: any = $state();

	let error = $state<string>();
	let errorCode = $state<string>();
	let confirmationSent = $state<boolean>(false);
	let resendCountdown = $state<number>(0);
	let resendDisabled = $state<boolean>(false);

	// Clear error when user starts typing
	function clearError() {
		error = undefined;
		errorCode = undefined;
		confirmationSent = false;
	}

	function startResendCountdown() {
		resendDisabled = true;
		resendCountdown = 30;

		const timer = setInterval(() => {
			resendCountdown--;
			if (resendCountdown <= 0) {
				clearInterval(timer);
				resendDisabled = false;
				resendCountdown = 0;
			}
		}, 1000);
	}

	const isFormValid = $derived(!!email && !!password && (!email || emailTextbox?.isValid()));

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	async function handleSubmit(event: Event) {
		event.preventDefault();

		// Clear previous errors when attempting to log in
		clearError();

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
			const url = new URL('successful_login', env.PUBLIC_APP_HOST);
			url.searchParams.set('access_token', encodeURIComponent(token));
			const path = url.toString();
			window.location.href = path;
		}
	}

	async function resendConfirmationEmail() {
		if (!email) {
			error = 'Please enter your email to resend the confirmation email.';
			return;
		}

		// Start countdown immediately when user clicks
		startResendCountdown();

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

<FullscreenIllustrationCard>
	{#snippet title()}
		<i>Login</i>
		to GitButler
	{/snippet}

	<div id="login-form" class="stack-v">
		<div class="auth-form__inputs">
			<EmailTextbox
				bind:this={emailTextbox}
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
			<InfoMessage filled outlined={false} style="success" class="m-b-16">
				{#snippet content()}
					<p>Confirmation email sent! Please check your inbox.</p>
				{/snippet}
			</InfoMessage>
		{:else if error}
			<div class="wiggle-animation">
				<InfoMessage filled outlined={false} style="danger" class="m-b-16">
					{#snippet content()}
						{#if errorCode === 'email_not_verified'}
							{#if !resendDisabled}
								<p>
									Verify your email before logging in. Check your inbox or <button
										type="button"
										class="resend-btn"
										onclick={resendConfirmationEmail}
										disabled={!email || resendDisabled}
									>
										resend the confirmation email</button
									>.
								</p>
							{:else}
								<p>
									Verify your email before logging in. You can resend the confirmation email in {resendCountdown}
									seconds.
								</p>
							{/if}
						{:else}
							<p>{error}</p>
						{/if}
					{/snippet}
				</InfoMessage>
			</div>
		{/if}

		<Button style="pop" disabled={!isFormValid} onclick={handleSubmit}>Log in</Button>

		<OAuthButtons mode="signup" />
	</div>

	{#snippet footer()}
		<div class="auth-form__footer">
			<p>
				Don't have an account? <a href={routesService.signupPath()}>Sign Up</a>
			</p>
		</div>
	{/snippet}
</FullscreenIllustrationCard>

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

	.auth-form__footer {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
	}

	.resend-btn {
		text-decoration: underline;
		text-decoration-style: dotted;
		cursor: pointer;
	}
</style>
