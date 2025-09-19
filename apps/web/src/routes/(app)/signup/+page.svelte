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

	const passwordsMatch = $derived(password === passwordConfirmation);

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
				label="Email"
				placeholder=" "
				bind:value={email}
				autocomplete={false}
				autocorrect={false}
				spellcheck
			/>
			<Textbox bind:value={password} label="Password" type="password" autocomplete />
			<Textbox
				bind:value={passwordConfirmation}
				label="Password confirmation"
				type="password"
				autocomplete
			/>
		</div>

		<Button type="submit" style="pop">Create account</Button>
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
