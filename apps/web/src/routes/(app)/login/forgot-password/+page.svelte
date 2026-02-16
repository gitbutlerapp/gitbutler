<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import FullscreenUtilityCard from '$lib/components/service/FullscreenUtilityCard.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, EmailTextbox, InfoMessage } from '@gitbutler/ui';

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	let email = $state<string>();
	let emailTextbox: any = $state();
	let error = $state<string>();
	let isLinkSent = $state<boolean>(false);
	let sentToEmail = $state<string>();

	const canSubmit = $derived(!!email && emailTextbox?.isValid());

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

<FullscreenUtilityCard
	title={isLinkSent ? 'Link sent!' : 'Forgot password?'}
	backlink={{ label: 'Login', href: routesService.loginPath() }}
>
	{#if isLinkSent}
		<p class="text-13 text-body">
			We've sent a password reset link to: <i class="clr-text-2">{sentToEmail}</i>
			<br />
			Click the link in your email to reset your password.
		</p>
	{:else}
		<div class="service-form__inputs">
			<EmailTextbox bind:this={emailTextbox} bind:value={email} label="Email" />

			{#if error}
				<InfoMessage filled outlined={false} style="danger">
					{#snippet content()}
						{error}
					{/snippet}
				</InfoMessage>
			{/if}

			<Button style="pop" disabled={!canSubmit} onclick={handleSubmit}>Send a reset link</Button>
		</div>
	{/if}
</FullscreenUtilityCard>

<style lang="postcss">
	.service-form__inputs {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
</style>
