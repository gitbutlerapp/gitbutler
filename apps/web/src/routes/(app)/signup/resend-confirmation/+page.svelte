<script lang="ts">
	import { page } from '$app/state';
	import AuthUtilityLayout from '$lib/components/auth/AuthUtilityLayout.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { Button, InfoMessage, EmailTextbox } from '@gitbutler/ui';

	let error = $state<string>();
	let message = $state<string>();

	const loginService = inject(LOGIN_SERVICE);

	const email = $derived(page.url.searchParams.get('email'));
	const messageCode = $derived(page.url.searchParams.get('message_code'));
	const banner = $derived(
		messageCode === 'invalid_or_expired_token'
			? 'It seems that your confirmation token is invalid or has expired. Please try resending the confirmation email.'
			: undefined
	);

	let inputEmail = $state<string>();

	const emailToSendTo = $derived(inputEmail ?? email ?? undefined);

	async function resendConfirmationEmail() {
		if (!emailToSendTo) {
			error = 'Please enter your email to resend the confirmation email.';
			return;
		}
		const response = await loginService.resendConfirmationEmail(emailToSendTo);
		if (response.type === 'error') {
			error = response.errorMessage;
			console.error('Failed to resend confirmation email:', response.raw ?? response.errorMessage);
		} else {
			message = 'Confirmation email resent. Please check your inbox.';
		}
	}
</script>

<svelte:head>
	<title>GitButler | Resend Confirmation</title>
</svelte:head>

<AuthUtilityLayout title="Resend Confirmation">
	{#if email}
		<p class="text-13 text-body">
			We send a confirmation email to <i class="clr-text-2">{email}</i>.
		</p>
	{:else}
		<div class="stack-v gap-16">
			<EmailTextbox
				bind:value={inputEmail}
				label="Email"
				helperText="Your email to resend confirmation"
			/>

			{#if error}
				<InfoMessage filled outlined={false} style="error">
					{#snippet content()}
						{error}
					{/snippet}
				</InfoMessage>
			{/if}

			{#if message}
				<InfoMessage filled outlined={false} style="success">
					{#snippet content()}
						{message}
					{/snippet}
				</InfoMessage>
			{/if}

			{#if banner}
				<InfoMessage filled outlined={false} style="warning">
					{#snippet content()}
						{banner}
					{/snippet}
				</InfoMessage>
			{/if}

			<Button style="pop" disabled={!emailToSendTo} onclick={resendConfirmationEmail}
				>Resend Confirmation Email</Button
			>
		</div>
	{/if}
</AuthUtilityLayout>
