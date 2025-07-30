<script lang="ts">
	import { page } from '$app/state';
	import { inject } from '@gitbutler/shared/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { Button, SectionCard } from '@gitbutler/ui';

	let error = $state<string>();
	let notice = $state<string>();

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
			notice = 'Confirmation email resent. Please check your inbox.';
		}
	}
</script>

<svelte:head>
	<title>GitButler | Sign Up</title>
</svelte:head>

{#if banner}
	<div class="banner">
		<span>{banner}</span>
	</div>
{/if}

<div class="resend-confirmation-email">
	<SectionCard>
		<h1 class="title">Resend Confirmation Email</h1>
		{#if email}
			<p>
				We will send a confirmation email to <strong>{email}</strong>.
			</p>
		{:else}
			<p>Please provide your email address to resend the confirmation email.</p>

			<div class="field">
				<label for="email">Email</label>
				<input id="email" type="email" bind:value={inputEmail} required />
			</div>
		{/if}

		<Button style="pop" disabled={!emailToSendTo} onclick={resendConfirmationEmail}
			>Resend Confirmation Email</Button
		>

		{#if error}
			<div class="error-message">
				<span>
					{error}
				</span>
			</div>
		{/if}

		{#if notice}
			<div class="notice-message">
				<span>
					{notice}
				</span>
			</div>
		{/if}
	</SectionCard>
</div>

<style lang="postcss">
	.banner {
		display: flex;
		align-items: center;
		justify-content: center;
		max-width: 400px;
		margin: 0 auto;
		margin-bottom: 16px;
		padding: 16px;
		border: var(--clr-scale-err-60) 1px solid;
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-err-bg-muted);
		color: var(--clr-scale-err-10);
		font-size: 14px;
	}

	.resend-confirmation-email {
		display: flex;
		flex-direction: column;
		max-width: 400px;
		margin: 0 auto;
		gap: 16px;
	}

	.title {
		align-self: flex-start;
		color: var(--clr-scale-ntrl-0);
		font-weight: 600;
		font-size: 24px;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;

		label {
			color: var(--clr-scale-ntrl-30);
			font-size: 14px;
		}

		input {
			padding: 8px 12px;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
			background-color: var(--clr-bg-1);
			color: var(--clr-scale-ntrl-0);
			font-size: 14px;

			&:read-only {
				cursor: not-allowed;
				opacity: 0.7;
			}

			&:not(:read-only) {
				&:focus {
					border-color: var(--clr-scale-pop-70);
					outline: none;
				}
			}
		}
	}

	.error-message {
		display: flex;
		flex-direction: column;
		padding: 8px;
		gap: 8px;
		border: 1px solid var(--clr-scale-err-60);
		border-radius: var(--radius-m);

		background-color: var(--clr-theme-err-bg-muted);
		color: var(--clr-scale-err-10);
		font-size: 14px;
	}

	.notice-message {
		display: flex;
		flex-direction: column;
		padding: 8px;
		gap: 8px;
		border: 1px solid var(--clr-scale-succ-60);
		border-radius: var(--radius-m);

		background-color: var(--clr-theme-succ-bg-muted);
		color: var(--clr-scale-succ-10);
		font-size: 14px;
	}
</style>
