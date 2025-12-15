<script lang="ts">
	import newProjectSvg from '$lib/assets/splash-illustrations/new-project.svg?raw';
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import OAuthButtons from '$lib/components/auth/OAuthButtons.svelte';
	import PasswordConfirmation from '$lib/components/auth/PasswordConfirmation.svelte';
	import UsernameTextbox from '$lib/components/auth/UsernameTextbox.svelte';
	import FullscreenIllustrationCard from '$lib/components/service/FullscreenIllustrationCard.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, EmailTextbox, InfoMessage } from '@gitbutler/ui';

	let username = $state<string>();
	let email = $state<string>();
	let password = $state<string>();
	let passwordConfirmation = $state<string>();
	let error = $state<string>();
	let successMessage = $state<string>();

	let emailTextbox: any = $state();
	let usernameTextbox: any = $state();
	let passwordComponent: PasswordConfirmation | undefined = $state();

	const isFormValid = $derived(
		username?.trim() &&
			email?.trim() &&
			emailTextbox?.isValid() &&
			usernameTextbox?.isValid() &&
			passwordComponent?.isValid?.()
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

		if (!usernameTextbox?.isValid()) {
			error = 'Please check your username';
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
			successMessage = response.data.message;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Sign Up</title>
</svelte:head>

<RedirectIfLoggedIn />

<FullscreenIllustrationCard illustration={successMessage ? newProjectSvg : undefined}>
	{#snippet title()}
		{#if !successMessage}
			<i>Sign Up</i>
			for GitButler
		{:else}
			🚀 Check <i>your email</i> for confirmation instructions
		{/if}
	{/snippet}

	{#if !successMessage}
		<form id="signup-form" class="stack-v" onsubmit={handleSubmit}>
			<div class="auth-form__inputs">
				<UsernameTextbox bind:this={usernameTextbox} bind:value={username} />
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
				<InfoMessage filled outlined={false} style="danger" class="m-b-16">
					{#snippet content()}
						{error}
					{/snippet}
				</InfoMessage>
			{/if}

			<Button type="submit" style="pop" disabled={!isFormValid}>Create account</Button>

			<OAuthButtons mode="signup" />
		</form>
	{/if}

	{#snippet footer()}
		<div class="auth-form__footer">
			{#if !successMessage}
				<p>
					*By signing up, you agree to our
					<a
						href="https://app.termly.io/document/terms-and-conditions/7924c71d-80bb-4444-9381-947237b9fc0f"
						>Terms</a
					>
					and
					<a
						href="https://app.termly.io/document/privacy-policy/a001c8b7-505b-4eab-81e3-fcd1c72bdd79"
						>Privacy Policy</a
					>
				</p>
				<p>
					Already have an account? <a href={routesService.loginPath()}>Log in now</a>
				</p>
			{:else}
				<p>
					Need help? <a
						href="https://github.com/gitbutlerapp/gitbutler/issues/new?template=BLANK_ISSUE"
						target="_blank"
						rel="noopener noreferrer"
					>
						Open a support request
					</a>
				</p>
			{/if}
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
