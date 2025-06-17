<script lang="ts">
	import { goto } from '$app/navigation';
	import { AuthService } from '$lib/auth/authService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import LoginService from '@gitbutler/shared/login/loginService';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';
	import { isStr } from '@gitbutler/ui/utils/string';

	let email = $state<string>();
	let password = $state<string>();

	let error = $state<string>();
	let errorCode = $state<string>();

	const loginService = getContext(LoginService);
	const routesService = getContext(WebRoutesService);
	const authService = getContext(AuthService);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		if (!email || !password) {
			console.error('Email and password are required');
			return;
		}

		const response = await loginService.loginWithEmail(email, password);

		if (response.type === 'error') {
			error = response.errorMessage;
			errorCode = response.errorCode;
			console.error('Login failed:', response.raw ?? response.errorMessage);
		} else {
			// Redirect to home or dashboard after successful login
			const searchParams = new URLSearchParams(window.location.search);
			searchParams.set('gb_access_token', response.data);
			const url = new URL(routesService.homeUrl());
			url.search = searchParams.toString();

			window.location.href = url.toString();
		}
	}

	async function resendConfirmationEmail() {
		if (!email) {
			error = 'Please enter your email to resend the confirmation email.';
			return;
		}
		const response = await loginService.resendConfirmationEmail(email);
		if (response.type === 'error') {
			error = response.errorMessage;
			errorCode = response.errorCode;
			console.error('Failed to resend confirmation email:', response.raw ?? response.errorMessage);
		} else {
			error = 'Confirmation email resent. Please check your inbox.';
		}
	}

	$effect(() => {
		if (isStr(authService.token.current)) {
			goto(routesService.homePath());
		}
	});
</script>

<svelte:head>
	<title>GitButler | Login</title>
</svelte:head>

<form onsubmit={handleSubmit} class="login-form">
	<div class="login-form__content">
		<SectionCard>
			<div class="field">
				<label for="email">Email</label>
				<input id="email" type="email" bind:value={email} required />
			</div>
			<div class="field">
				<label for="password">Password</label>
				<input id="password" type="password" bind:value={password} required />
			</div>
		</SectionCard>

		<Button type="submit">Log in</Button>

		{#if error}
			<div class="error-message">
				<span>
					{error}
				</span>
			</div>
			{#if errorCode === 'email_not_verified'}
				<span>
					Please verify your email address before logging in. Check your inbox for a verification
					email or
				</span>
				<Button
					type="button"
					onclick={resendConfirmationEmail}
					disabled={!email}
					tooltip={!email
						? 'Please enter your email in the field above to resend the confirmation email.'
						: 'Resend confirmation email'}
				>
					resend the confirmation email</Button
				>
			{/if}
		{/if}
	</div>
</form>

<style lang="postcss">
	.login-form__content {
		display: flex;
		flex-direction: column;
		max-width: 400px;
		margin: auto;
		gap: 16px;
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
</style>
