<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import OAuthButtons from '$lib/components/auth/OAuthButtons.svelte';
	import PasswordConfirmation from '$lib/components/auth/PasswordConfirmation.svelte';
	import FullscreenIllustrationCard from '$lib/components/service/FullscreenIllustrationCard.svelte';
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

	let emailTextbox: any = $state();
	let passwordComponent: PasswordConfirmation | undefined = $state();

	const isFormValid = $derived(
		username?.trim() && email?.trim() && emailTextbox?.isValid() && passwordComponent?.isValid?.()
	);

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		if (!username || !email || !password || !passwordConfirmation) {
			error = 'Username, email and password are required';
			return;
		}

		if (!passwordComponent?.isValid()) {
			error = 'Please check your password and confirmation';
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

<FullscreenIllustrationCard>
	{#snippet title()}
		<i>Sign Up</i>
		for GitButler
	{/snippet}

	{#if message}
		<InfoMessage filled outlined={false} style="success" class="m-bottom-16">
			{#snippet content()}
				{message}
			{/snippet}
		</InfoMessage>
	{:else}
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
				<PasswordConfirmation
					bind:this={passwordComponent}
					bind:password
					bind:passwordConfirmation
				/>
			</div>

			{#if error}
				<InfoMessage filled outlined={false} style="error" class="m-bottom-16">
					{#snippet content()}
						{error}
					{/snippet}
				</InfoMessage>
			{/if}

			<Button type="submit" style="pop" disabled={!isFormValid}>Create account</Button>
		</form>
	{/if}

	<OAuthButtons mode="signup" />

	{#snippet footer()}
		<div class="auth-form__footer">
			<p>
				*By signing up, you agree to our
				<a
					href="https://app.termly.io/document/terms-and-conditions/7924c71d-80bb-4444-9381-947237b9fc0f"
					>Terms</a
				>
				and
				<a href="https://app.termly.io/document/privacy-policy/a001c8b7-505b-4eab-81e3-fcd1c72bdd79"
					>Privacy Policy</a
				>
			</p>
			<p>
				Already have an account? <a href={routesService.loginPath()}>Log in now</a>
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

	.auth-form__footer {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
	}
</style>
