<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import PasswordPageLayout from '$lib/components/auth/PasswordPageLayout.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { Button, EmailTextbox, InfoMessage } from '@gitbutler/ui';

	const loginService = inject(LOGIN_SERVICE);

	let email = $state<string>();
	let error = $state<string>();
	let isLinkSent = $state<boolean>(false);
	let sentToEmail = $state<string>();

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
			sentToEmail = email;
			isLinkSent = true;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Forgot Password</title>
</svelte:head>

<RedirectIfLoggedIn />

<PasswordPageLayout title={isLinkSent ? 'Link sent!' : 'Forgot password?'}>
	{#if isLinkSent}
		<p class="text-13 text-body">
			We've sent a password reset link to: <i class="clr-text-2">{sentToEmail}</i>
			<br />
			Click the link in your email to reset your password.
		</p>
	{:else}
		<div class="service-form__inputs">
			<EmailTextbox bind:value={email} label="Email" />
			<Button style="pop" type="submit" onclick={handleSubmit}>Send a reset link</Button>
		</div>

		{#if error}
			<InfoMessage filled outlined={false} style="error" class="m-top-16">
				{#snippet content()}
					{error}
				{/snippet}
			</InfoMessage>
		{/if}
	{/if}
</PasswordPageLayout>

<style lang="postcss">
	.service-form__inputs {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>
