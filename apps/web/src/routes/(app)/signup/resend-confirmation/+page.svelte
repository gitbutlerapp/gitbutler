<script lang="ts">
	import { page } from '$app/state';
	import FullscreenUtilityCard from '$lib/components/service/FullscreenUtilityCard.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, InfoMessage, EmailTextbox } from '@gitbutler/ui';

	let error = $state<string>();
	let message = $state<string>();

	let emailTextbox: any = $state();

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	const email = $derived(page.url.searchParams.get('email'));
	const messageCode = $derived(page.url.searchParams.get('message_code'));
	const banner = $derived(
		messageCode === 'invalid_or_expired_token'
			? 'It seems that your confirmation token is invalid or has expired. Please try resending the confirmation email.'
			: undefined
	);

	let inputEmail = $state<string>();

	const emailToSendTo = $derived(inputEmail ?? email ?? undefined);
	const isValidEmail = $derived(email ? true : !inputEmail || emailTextbox?.isValid());
	const canSubmit = $derived(!!emailToSendTo && isValidEmail);

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

<FullscreenUtilityCard
	title="Resend Confirmation"
	backlink={{ label: 'Login', href: routesService.loginPath() }}
>
	{#if email}
		<p class="text-13 text-body">
			We send a confirmation email to <i class="clr-text-2">{email}</i>.
		</p>
	{:else}
		<form class="stack-v gap-16" onsubmit={resendConfirmationEmail}>
			<EmailTextbox bind:this={emailTextbox} bind:value={inputEmail} label="Email" />

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

			<Button style="pop" disabled={!canSubmit}>Resend Confirmation Email</Button>
		</form>
	{/if}
</FullscreenUtilityCard>
