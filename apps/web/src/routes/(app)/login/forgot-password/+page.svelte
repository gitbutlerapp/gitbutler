<script lang="ts">
	import RedirectIfLoggedIn from '$lib/auth/RedirectIfLoggedIn.svelte';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, EmailTextbox } from '@gitbutler/ui';

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	let email = $state<string>();
	let error = $state<string>();
	let message = $state<string>();

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
			message = response.data.message;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Forgot Password</title>
</svelte:head>

<RedirectIfLoggedIn />

<div class="service-form__page">
	<form onsubmit={handleSubmit} class="service-form">
		<h1 class="text-serif-42 m-bottom-20">Forgot password?</h1>

		<div class="service-form__inputs">
			<EmailTextbox bind:value={email} label="Email" />
			<Button style="pop" type="submit">Send a reset link</Button>
		</div>

		<div class="text-12 service-form__footer">
			<p>
				← Back to
				<a href={routesService.loginPath()}>Log in</a>
			</p>
			<p>
				Need help?
				<a
					href="https://github.com/gitbutlerapp/gitbutler/issues/new?template=BLANK_ISSUE"
					target="_blank"
					rel="noopener noreferrer"
				>
					Open a support request
				</a>
			</p>
		</div>

		{#if error}
			<div class="error-message">{error}</div>
		{/if}

		{#if message}
			<div class="message">{message}</div>
		{/if}
	</form>
</div>

<style lang="postcss">
	.service-form__page {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
	}

	.service-form {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 540px;
		padding: 50px 60px 40px;
		border-radius: var(--radius-xl);
		background-color: var(--clr-bg-1);
	}

	.service-form__inputs {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.service-form__footer {
		display: flex;
		justify-content: space-between;
		margin-top: 40px;
		color: var(--clr-text-2);
		text-align: center;

		a {
			text-decoration: underline;
			transition: color var(--transition-fast);

			&:hover {
				color: var(--clr-text-1);
			}
		}
	}

	@media (max-width: 600px) {
		.service-form {
			padding: 30px 20px 20px;
		}

		.service-form__footer {
			flex-direction: column;
			margin-top: 24px;
			gap: 8px;
		}
	}
</style>
