<script lang="ts">
	import { page } from "$app/state";
	import FullscreenUtilityCard from "$lib/components/service/FullscreenUtilityCard.svelte";
	import { inject } from "@gitbutler/core/context";
	import { LOGIN_SERVICE } from "@gitbutler/shared/login/loginService";
	import { WEB_ROUTES_SERVICE } from "@gitbutler/shared/routing/webRoutes.svelte";
	import { Button, InfoMessage, EmailTextbox } from "@gitbutler/ui";

	let error = $state<string>();
	let message = $state<string>();

	let emailTextbox: any = $state();

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	const email = $derived(page.url.searchParams.get("email"));
	const messageCode = $derived(page.url.searchParams.get("message_code"));
	const banner = $derived(
		messageCode === "invalid_or_expired_token"
			? "Es scheint, dass dein Bestätigungstoken ungültig ist oder abgelaufen ist. Bitte versuche, die Bestätigungs-E-Mail erneut zu senden."
			: undefined,
	);

	let inputEmail = $state<string>();

	const emailToSendTo = $derived(inputEmail ?? email ?? undefined);
	const isValidEmail = $derived(email ? true : !inputEmail || emailTextbox?.isValid());
	const canSubmit = $derived(!!emailToSendTo && isValidEmail);

	async function resendConfirmationEmail() {
		if (!emailToSendTo) {
			error = "Bitte gib deine E-Mail-Adresse ein, um die Bestätigungs-E-Mail erneut zu senden.";
			return;
		}
		const response = await loginService.resendConfirmationEmail(emailToSendTo);
		if (response.type === "error") {
			error = response.errorMessage;
			console.error("Failed to resend confirmation email:", response.raw ?? response.errorMessage);
		} else {
			message = "Bestätigungs-E-Mail erneut gesendet. Bitte überprüfe deinen Posteingang.";
		}
	}
</script>

<svelte:head>
	<title>GitButler | Bestätigung erneut senden</title>
</svelte:head>

<FullscreenUtilityCard
	title="Bestätigung erneut senden"
	backlink={{ label: "Anmelden", href: routesService.loginPath() }}
>
	{#if email}
		<p class="text-13 text-body">
			Wir senden eine Bestätigungs-E-Mail an <i class="clr-text-2">{email}</i>.
		</p>
	{:else}
		<div class="stack-v gap-16">
			<EmailTextbox bind:this={emailTextbox} bind:value={inputEmail} label="E-Mail" />

			{#if error}
				<InfoMessage filled outlined={false} style="danger">
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

			<Button style="pop" disabled={!canSubmit} onclick={resendConfirmationEmail}
				>Bestätigungs-E-Mail erneut senden</Button
			>
		</div>
	{/if}
</FullscreenUtilityCard>
