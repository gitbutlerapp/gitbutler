<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import LoginService from '@gitbutler/shared/login/loginService';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';

	let email = $state<string>();
	let password = $state<string>();
	let passwordConfirmation = $state<string>();
	let error = $state<string>();

	const passwordsMatch = $derived(password === passwordConfirmation);

	const loginService = getContext(LoginService);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		if (!email || !password || !passwordConfirmation) {
			error = 'Email and password are required';
			return;
		}

		if (!passwordsMatch) {
			error = 'Passwords do not match';
			return;
		}

		const response = await loginService.createAccountWithEmail(
			email,
			password,
			passwordConfirmation
		);

		if (response.type === 'error') {
			error = response.errorMessage;
			console.error('Login failed:', response.raw ?? response.errorMessage);
		} else {
			// Redirect to home or dashboard after successful sign in
			window.location.href = '/';
		}
	}
</script>

<svelte:head>
	<title>GitButler | Sign Up</title>
</svelte:head>

<form onsubmit={handleSubmit} class="signin-form">
	<div class="signup-form__content">
		<SectionCard>
			<div class="field">
				<label for="email">Email</label>
				<input id="email" type="email" bind:value={email} required />
			</div>
			<div class="field">
				<label for="password">Password</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					required
					autocomplete="current-password"
				/>
			</div>
			<div class="field">
				<label for="passwordConfirmation">Password confirmation</label>
				<input
					id="passwordConfirmation"
					type="password"
					bind:value={passwordConfirmation}
					required
					autocomplete="new-password"
				/>
			</div>
		</SectionCard>

		<Button type="submit">Sign in</Button>

		{#if error}
			<div class="error-message">{error}</div>
		{/if}
	</div>
</form>

<style lang="postcss">
	.signup-form__content {
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
		padding: 8px;
		border: 1px solid var(--clr-scale-err-60);
		border-radius: var(--radius-m);
		background-color: var(--clr-theme-err-bg-muted);
		color: var(--clr-scale-err-10);
		font-size: 14px;
	}
</style>
