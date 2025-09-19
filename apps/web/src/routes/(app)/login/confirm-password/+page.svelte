<script lang="ts">
	import { page } from '$app/state';
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import PasswordConfirmation from '$lib/components/auth/PasswordConfirmation.svelte';
	import PasswordPageLayout from '$lib/components/auth/PasswordPageLayout.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { Button, InfoMessage } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';

	const loginService = inject(LOGIN_SERVICE);
	const authService = inject(AUTH_SERVICE);

	let password = $state<string>();
	let passwordConfirmation = $state<string>();
	let error = $state<string>();
	let message = $state<string>();
	let passwordComponent: PasswordConfirmation | undefined;

	async function handleSubmit() {
		const token = page.url.searchParams.get('t');

		if (!token) {
			error = 'Invalid or missing token';
			// TODO: Probably redirect to the login page or show a more user-friendly error
			return;
		}

		if (!passwordComponent?.isValid()) {
			error = 'Please check your password and confirmation';
			return;
		}

		if (!password || !passwordConfirmation) {
			error = 'Password are required';
			return;
		}

		const response = await loginService.confirmPasswordReset(token, password, passwordConfirmation);
		if (response.type === 'error') {
			error = response.errorMessage;
			console.error('Confirm password failed:', response.raw ?? response.errorMessage);
			message = '';
			return;
		}

		error = undefined;
		message = response.data.message;
		authService.setToken(response.data.token);
		window.location.href = `${env.PUBLIC_APP_HOST}successful_login?access_token=${token}`;
	}
</script>

<svelte:head>
	<title>GitButler | Confirm Password</title>
</svelte:head>

<RedirectIfLoggedIn />

<PasswordPageLayout title="Confirm new password">
	<div class="form-content">
		<PasswordConfirmation
			bind:this={passwordComponent}
			bind:password
			bind:passwordConfirmation
			showValidation={true}
		/>

		<Button type="submit" style="pop" onclick={handleSubmit}>Confirm Password</Button>

		{#if error}
			<InfoMessage filled outlined={false} style="error" class="m-top-16">
				{#snippet content()}
					{error}
				{/snippet}
			</InfoMessage>
		{/if}

		{#if message}
			<InfoMessage filled outlined={false} style="success" class="m-top-16">
				{#snippet content()}
					{message}
				{/snippet}
			</InfoMessage>
		{/if}
	</div>
</PasswordPageLayout>

<style lang="postcss">
	.form-content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>
