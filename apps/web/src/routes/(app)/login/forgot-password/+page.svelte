<script lang="ts">
	import RedirectIfLoggedIn from "$lib/auth/RedirectIfLoggedIn.svelte";
	import FullscreenUtilityCard from "$lib/components/service/FullscreenUtilityCard.svelte";
	import { inject } from "@gitbutler/core/context";
	import { LOGIN_SERVICE } from "@gitbutler/shared/login/loginService";
	import { WEB_ROUTES_SERVICE } from "@gitbutler/shared/routing/webRoutes.svelte";
	import { Button, EmailTextbox, InfoMessage } from "@gitbutler/ui";

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
			error = "E-Mail-Adresse ist erforderlich";
			return;
		}

		const response = await loginService.resetPassword(email);
		if (response.type === "error") {
			error = response.errorMessage;
			console.error("Reset password failed:", response.raw ?? response.errorMessage);
		} else {
			error = undefined;
			sentToEmail = email;
			isLinkSent = true;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Passwort vergessen</title>
</svelte:head>

<RedirectIfLoggedIn />

<FullscreenUtilityCard
	title={isLinkSent ? "Link gesendet!" : "Passwort vergessen?"}
	backlink={{ label: "Anmelden", href: routesService.loginPath() }}
>
	{#if isLinkSent}
		<p class="text-13 text-body">
			Wir haben einen Link zum Zurücksetzen des Passworts gesendet an: <i class="clr-text-2">{sentToEmail}</i>
			<br />
			Klicke auf den Link in deiner E-Mail, um dein Passwort zurückzusetzen.
		</p>
	{:else}
		<div class="service-form__inputs">
			<EmailTextbox bind:this={emailTextbox} bind:value={email} label="E-Mail" />

			{#if error}
				<InfoMessage filled outlined={false} style="danger">
					{#snippet content()}
						{error}
					{/snippet}
				</InfoMessage>
			{/if}

			<Button style="pop" disabled={!canSubmit} onclick={handleSubmit}>Link zum Zurücksetzen senden</Button>
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
