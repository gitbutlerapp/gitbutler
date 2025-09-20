<script lang="ts">
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import FullscreenIllustrationCard from '$lib/components/service/FullscreenIllustrationCard.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import { LOGIN_SERVICE } from '@gitbutler/shared/login/loginService';
	import { Button, InfoMessage, Textbox, EmailTextbox } from '@gitbutler/ui';

	const userService = inject(USER_SERVICE);
	const loginService = inject(LOGIN_SERVICE);
	const user = userService.user;
	const userEmail = $derived($user?.email);
	const userLogin = $derived($user?.login);
	const authService = inject(AUTH_SERVICE);
	const token = authService.tokenReadable;

	let email = $state<string>();
	let username = $state<string>();

	let emailTextbox: any = $state();

	let error = $state<string>();
	let message = $state<string>();
	const effectiveEmail = $derived(email ?? userEmail);
	const effectiveUsername = $derived(username ?? userLogin);
	const canSubmit = $derived(
		!!effectiveEmail && !!effectiveUsername && (!email || emailTextbox?.isValid())
	);

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

	// $effect(() => {
	// 	if (!isLoggedIn) {
	// 		goto(routesService.loginPath());
	// 	} else if (isFinalized) {
	// 		goto(routesService.homePath());
	// 	}
	// });
</script>

<svelte:head>
	<title>GitButler | Finalize Account</title>
</svelte:head>

<FullscreenIllustrationCard>
	{#snippet title()}
		Almost <i>done</i>!
	{/snippet}

	<form class="finalize-form__content" onsubmit={handleSubmit}>
		<p class="text-12 text-base finalize-form__caption">
			We need these details to set up your account properly.
		</p>
		{#if !userLogin}
			<Textbox bind:value={username} label="Username" />
		{/if}
		{#if !userEmail}
			<EmailTextbox bind:this={emailTextbox} bind:value={email} label="Email" />
		{/if}

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

		<Button style="pop" type="submit" disabled={!canSubmit}>Finalize Account</Button>
	</form>
</FullscreenIllustrationCard>

<style lang="postcss">
	.finalize-form__content {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.finalize-form__caption {
		margin-top: -20px;
		margin-bottom: 8px;
		color: var(--clr-text-2);
	}
</style>
