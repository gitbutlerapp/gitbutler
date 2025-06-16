<script lang="ts">
	import { getContext } from '@gitbutler/shared/context';
	import LoginService from '@gitbutler/shared/login/loginService';
	import Button from '@gitbutler/ui/Button.svelte';
	import SectionCard from '@gitbutler/ui/SectionCard.svelte';

	let email = $state<string>();
	let password = $state<string>();

	let error = $state<string>();

	const loginService = getContext(LoginService);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		// TODO: handle login logic
		console.warn('Email:', email, 'Password:', password);
		if (!email || !password) {
			console.error('Email and password are required');
			return;
		}
		const response = await loginService.loginWithEmail(email, password);
		if (response.type === 'error') {
			error = response.errorMessage;
			console.error('Login failed:', response.raw ?? response.errorMessage);
		} else {
			// Redirect to home or dashboard after successful login
			window.location.href = '/';
		}
	}
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
			<div class="error-message">{error}</div>
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
		padding: 8px;
		border: 1px solid var(--clr-scale-err-60);
		border-radius: var(--radius-m);

		background-color: var(--clr-theme-err-bg-muted);
		color: var(--clr-scale-err-10);
		font-size: 14px;
	}
</style>
