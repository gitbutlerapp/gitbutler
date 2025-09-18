<script lang="ts">
	import { goto } from '$app/navigation';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, SectionCard } from '@gitbutler/ui';

	const userService = inject(USER_SERVICE);
	const loginService = inject(LOGIN_SERVICE);
	const user = userService.user;
	const isLoggedIn = $derived($user !== undefined);
	const userEmail = $derived($user?.email);
	const userLogin = $derived($user?.login);
	const authService = inject(AUTH_SERVICE);
	const token = authService.tokenReadable;

	const isFinalized = $derived(isLoggedIn && userEmail && userLogin);
	const routesService = inject(WEB_ROUTES_SERVICE);

	let agreeToTerms = $state<boolean>(false);
	let email = $state<string>();
	let username = $state<string>();

	let error = $state<string>();
	let message = $state<string>();
	const effectiveEmail = $derived(email ?? userEmail);
	const effectiveUsername = $derived(username ?? userLogin);
	const canSubmit = $derived(agreeToTerms && !!effectiveEmail && !!effectiveUsername);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		if (!$token) {
			// should not happen
			error = 'You must be logged in to finalize your account.';
			return;
		}

		if (!effectiveEmail) {
			error = 'Email is required.';
			return;
		}

		if (!effectiveUsername) {
			error = 'Username is required.';
			return;
		}

		if (!agreeToTerms) {
			error = 'You must agree to the terms and conditions.';
			return;
		}

		const response = await loginService.finalizeAccount($token, effectiveEmail, effectiveUsername);
		if (response.type === 'error') {
			error = response.errorMessage;
			console.error('Finalize account failed:', response.raw ?? response.errorMessage);
			return;
		}

		error = undefined;
		message = response.data.message;
		await userService.refreshUser();
	}

	$effect(() => {
		if (!isLoggedIn) {
			goto(routesService.loginPath());
		} else if (isFinalized) {
			goto(routesService.homePath());
		}
	});
</script>

<svelte:head>
	<title>GitButler | Login</title>
</svelte:head>

<form onsubmit={handleSubmit} class="finalize-form">
	<div class="finalize-form__content">
		<SectionCard>
			{#if !userLogin}
				<div class="field">
					<label for="username">Username</label>
					<input id="username" type="text" bind:value={username} required />
				</div>
			{/if}
			{#if !userEmail}
				<div class="field">
					<label for="email">Email</label>
					<input id="email" type="email" bind:value={email} required />
				</div>
			{/if}
		</SectionCard>
		<div class="terms">
			<label for="comments" class="text-13">Terms and Conditions Agreement</label>
			<div class="terms-checkbox">
				<input
					id="terms"
					name="terms"
					type="checkbox"
					value={agreeToTerms}
					onchange={(e) => (agreeToTerms = e.currentTarget.checked)}
				/>
				<p class="text-12">
					I agree to GitButler's
					<a
						href="https://app.termly.io/document/terms-and-conditions/7924c71d-80bb-4444-9381-947237b9fc0f"
						>Terms and Conditions</a
					>
					and
					<a
						href="https://app.termly.io/document/privacy-policy/a001c8b7-505b-4eab-81e3-fcd1c72bdd79"
						>Privacy Policy</a
					>
				</p>
			</div>
		</div>

		<Button style="pop" type="submit" disabled={!canSubmit}>🔥FINISH IT🔥</Button>

		{#if error}
			<div class="error-message">
				<span>
					{error}
				</span>
			</div>
		{/if}

		{#if message}
			<div class="message">{message}</div>
		{/if}
	</div>
</form>

<style lang="postcss">
	.finalize-form__content {
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

	.terms {
		display: flex;
		flex-direction: column;
		padding: 4px;
		gap: 8px;
	}

	.terms-checkbox {
		display: flex;
		align-items: center;
		gap: 8px;
		& a {
			text-decoration: underline;
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

	.message {
		padding: 8px;
		border: 1px solid var(--clr-scale-succ-60);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-succ-bg-muted);
		color: var(--clr-scale-err-10);
		font-size: 14px;
	}
</style>
