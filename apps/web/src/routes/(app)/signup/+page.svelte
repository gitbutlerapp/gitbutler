<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import AuthPageLayout from '$lib/components/auth/AuthPageLayout.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, EmailTextbox, Textbox, InfoMessage } from '@gitbutler/ui';

	let username = $state<string>();
	let email = $state<string>();
	let password = $state<string>();
	let passwordConfirmation = $state<string>();
	let error = $state<string>();
	let message = $state<string>();

	let emailTextbox: any;
	let passwordTouched = $state(false);
	let passwordConfirmationTouched = $state(false);

	const passwordsMatch = $derived(password === passwordConfirmation);

	function validatePassword(pwd: string) {
		if (!pwd) return { isValid: false, errors: [] };

		const errors = [];

		// Length check (minimum 8 characters)
		if (pwd.length < 8) {
			errors.push('at least 8 characters');
		}

		// Must contain at least one lowercase letter
		if (!/[a-z]/.test(pwd)) {
			errors.push('one lowercase letter');
		}

		// Must contain at least one uppercase letter
		if (!/[A-Z]/.test(pwd)) {
			errors.push('one uppercase letter');
		}

		// Must contain at least one number
		if (!/\d/.test(pwd)) {
			errors.push('one number');
		}

		return { isValid: errors.length === 0, errors };
	}

	const passwordValidation = $derived(validatePassword(password || ''));
	const isPasswordValid = $derived(passwordValidation.isValid);
	const passwordError = $derived(
		passwordTouched && password && !isPasswordValid
			? `Password must contain: ${passwordValidation.errors.join(', ')}`
			: undefined
	);
	const passwordHelperText = $derived(
		password && isPasswordValid
			? 'Strong password! ✅'
			: '8+ characters with uppercase, lowercase, and number'
	);
	const passwordConfirmationError = $derived(
		passwordConfirmationTouched && passwordConfirmation && !passwordsMatch
			? 'Passwords do not match'
			: undefined
	);

	const isFormValid = $derived(
		username?.trim() &&
			email?.trim() &&
			emailTextbox?.isValid() &&
			isPasswordValid &&
			passwordConfirmation?.trim() &&
			passwordsMatch
	);

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		if (!username || !email || !password || !passwordConfirmation) {
			error = 'Username, email and password are required';
			return;
		}

		if (!passwordsMatch) {
			error = 'Passwords do not match';
			return;
		}

		const response = await loginService.createAccountWithEmail(
			username,
			email,
			password,
			passwordConfirmation
		);

		if (response.type === 'error') {
			error = response.errorMessage;
			console.error('Login failed:', response.raw ?? response.errorMessage);
		} else {
			error = undefined;
			message = response.data.message;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Sign Up</title>
</svelte:head>

<RedirectIfLoggedIn />

<AuthPageLayout
	title="Sign Up"
	subtitle="for GitButler"
	oauthText="Or sign up with"
	bottomLinkText="Already have an account?"
	bottomLinkHref={routesService.loginPath()}
	bottomLinkLabel="Log in"
>
	<form id="signup-form" class="stack-v" onsubmit={handleSubmit}>
		<div class="auth-form__inputs">
			<Textbox bind:value={username} label="Username" />
			<EmailTextbox
				bind:this={emailTextbox}
				label="Email"
				placeholder=" "
				bind:value={email}
				autocomplete={false}
				autocorrect={false}
				spellcheck
			/>
			<Textbox
				bind:value={password}
				label="Password"
				type="password"
				autocomplete
				error={passwordError}
				helperText={passwordHelperText}
				onblur={() => {
					passwordTouched = true;
				}}
			/>
			<Textbox
				bind:value={passwordConfirmation}
				label="Confirm password"
				type="password-non-visible"
				autocomplete
				error={passwordConfirmationError}
				oninput={() => {
					passwordConfirmationTouched = true;
				}}
				onblur={() => {
					passwordConfirmationTouched = true;
				}}
			/>
		</div>

		<Button type="submit" style="pop" disabled={!isFormValid}>Create account</Button>
	</form>

	{#if error}
		<InfoMessage filled outlined={false} style="error" class="m-top-16">
			{#snippet content()}
				<p>{error}</p>
			{/snippet}
		</InfoMessage>
	{/if}

	{#if message}
		<InfoMessage filled outlined={false} style="success" class="m-top-16">
			{#snippet content()}
				<p>{message}</p>
			{/snippet}
		</InfoMessage>
	{/if}
</AuthPageLayout>

<style lang="postcss">
	.auth-form__inputs {
		display: flex;
		flex-direction: column;
		margin-bottom: 24px;
		gap: 14px;
	}
</style>
