<script lang="ts">
	import newProjectSvg from "$lib/assets/splash-illustrations/new-project.svg?raw";
	import RedirectIfLoggedIn from "$lib/auth/RedirectIfLoggedIn.svelte";
	import OAuthButtons from "$lib/components/auth/OAuthButtons.svelte";
	import PasswordConfirmation from "$lib/components/auth/PasswordConfirmation.svelte";
	import UsernameTextbox from "$lib/components/auth/UsernameTextbox.svelte";
	import FullscreenIllustrationCard from "$lib/components/service/FullscreenIllustrationCard.svelte";
	import { inject } from "@gitbutler/core/context";
	import { LOGIN_SERVICE } from "@gitbutler/shared/login/loginService";
	import { WEB_ROUTES_SERVICE } from "@gitbutler/shared/routing/webRoutes.svelte";
	import { Button, EmailTextbox, InfoMessage } from "@gitbutler/ui";

	let username = $state<string>();
	let email = $state<string>();
	let password = $state<string>();
	let passwordConfirmation = $state<string>();
	let error = $state<string>();
	let successMessage = $state<string>();

	let emailTextbox: any = $state();
	let usernameTextbox: any = $state();
	let passwordComponent: PasswordConfirmation | undefined = $state();

	const isFormValid = $derived(
		username?.trim() &&
			email?.trim() &&
			emailTextbox?.isValid() &&
			usernameTextbox?.isValid() &&
			passwordComponent?.isValid?.(),
	);

	const loginService = inject(LOGIN_SERVICE);
	const routesService = inject(WEB_ROUTES_SERVICE);

	async function handleSubmit(event: Event) {
		event.preventDefault();
		if (!username || !email || !password || !passwordConfirmation) {
			error = "Benutzername, E-Mail-Adresse und Passwort sind erforderlich";
			return;
		}

		if (!passwordComponent?.isValid()) {
			error = "Bitte überprüfe dein Passwort und die Bestätigung";
			return;
		}

		if (!usernameTextbox?.isValid()) {
			error = "Bitte überprüfe deinen Benutzernamen";
			return;
		}

		const response = await loginService.createAccountWithEmail(
			username,
			email,
			password,
			passwordConfirmation,
		);

		if (response.type === "error") {
			error = response.errorMessage;
			console.error("Login failed:", response.raw ?? response.errorMessage);
		} else {
			error = undefined;
			successMessage = response.data.message;
		}
	}
</script>

<svelte:head>
	<title>GitButler | Registrieren</title>
</svelte:head>

<RedirectIfLoggedIn />

<FullscreenIllustrationCard illustration={successMessage ? newProjectSvg : undefined}>
	{#snippet title()}
		{#if !successMessage}
			<i>Registrieren</i>
			für GitButler
		{:else}
			🚀 Überprüfe <i>deine E-Mails</i> für Anweisungen zur Bestätigung
		{/if}
	{/snippet}

	{#if !successMessage}
		<form id="signup-form" class="stack-v" onsubmit={handleSubmit}>
			<div class="auth-form__inputs">
				<UsernameTextbox bind:this={usernameTextbox} bind:value={username} />
				<EmailTextbox
					bind:this={emailTextbox}
					label="E-Mail"
					placeholder=" "
					bind:value={email}
					autocomplete={false}
					autocorrect={false}
					spellcheck
				/>
				<PasswordConfirmation
					bind:this={passwordComponent}
					bind:password
					bind:passwordConfirmation
				/>
			</div>

			{#if error}
				<InfoMessage filled outlined={false} style="danger" class="m-b-16">
					{#snippet content()}
						{error}
					{/snippet}
				</InfoMessage>
			{/if}

			<Button type="submit" style="pop" disabled={!isFormValid}>Konto erstellen</Button>

			<OAuthButtons mode="signup" />
		</form>
	{/if}

	{#snippet footer()}
		<div class="auth-form__footer">
			{#if !successMessage}
				<p>
					*Mit der Registrierung stimmst du unseren
					<a href="https://gitbutler.com/terms">Nutzungsbedingungen</a>
					und der
					<a href="https://gitbutler.com/privacy">Datenschutzerklärung</a> zu
				</p>
				<p>
					Du hast bereits ein Konto? <a href={routesService.loginPath()}>Jetzt anmelden</a>
				</p>
			{:else}
				<p>
					Brauchst du Hilfe? <a
						href="https://github.com/gitbutlerapp/gitbutler/issues/new?template=BLANK_ISSUE"
						target="_blank"
						rel="noopener noreferrer"
					>
						Support-Anfrage stellen
					</a>
				</p>
			{/if}
		</div>
	{/snippet}
</FullscreenIllustrationCard>

<style lang="postcss">
	.auth-form__inputs {
		display: flex;
		flex-direction: column;
		margin-bottom: 24px;
		gap: 14px;
	}

	.auth-form__footer {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 8px;
	}
</style>
