<script lang="ts">
	import ExperimentalSettings from "./components/ExperimentalSettings.svelte";
	import NotificationSettings from "./components/NotificationSettings.svelte";
	import ProfileHeader from "./components/ProfileHeader.svelte";
	import SupporterCard from "./components/SupporterCard.svelte";
	import linksJson from "$lib/data/links.json";
	import { USER_SERVICE } from "$lib/user/userService";
	import { getOS } from "$lib/utils/getOS";
	import { inject } from "@gitbutler/core/context";
	import { LOGIN_SERVICE } from "@gitbutler/shared/login/loginService";
	import Loading from "@gitbutler/shared/network/Loading.svelte";
	import { getRecentlyPushedProjects } from "@gitbutler/shared/organizations/projectsPreview.svelte";
	import { APP_STATE } from "@gitbutler/shared/redux/store.svelte";
	import { NOTIFICATION_SETTINGS_SERVICE } from "@gitbutler/shared/settings/notificationSettingsService";
	import { getNotificationSettingsInterest } from "@gitbutler/shared/settings/notificationSetttingsPreview.svelte";
	import { Button, CardGroup, chipToasts, Icon, Modal, Spacer } from "@gitbutler/ui";
	import { copyToClipboard } from "@gitbutler/ui/utils/clipboard";
	import { env } from "$env/dynamic/public";

	const userService = inject(USER_SERVICE);
	const appState = inject(APP_STATE);
	const notificationSettingsService = inject(NOTIFICATION_SETTINGS_SERVICE);
	const recentProjects = getRecentlyPushedProjects();
	const loginService = inject(LOGIN_SERVICE);

	const notificationSettings = getNotificationSettingsInterest(
		appState,
		notificationSettingsService,
	);

	const user = $derived(userService.user);

	const detectedOS = $derived.by(() => {
		const os = getOS();
		return os === "unknown" ? "macOS" : os;
	});

	const downloadButtonText = $derived(`GitButler für ${detectedOS} herunterladen`);

	const downloadLink = $derived.by(() => {
		switch (detectedOS) {
			case "Windows":
				return linksJson.downloads.windowsMsi.url;
			case "Linux":
				return linksJson.downloads.linuxAppimage.url;
			case "macOS":
			default:
				return linksJson.downloads.appleSilicon.url;
		}
	});

	async function refreshAccessToken() {
		await userService.refreshAccessToken();
		chipToasts.success("Zugriffstoken erfolgreich aktualisiert");
	}

	function logout() {
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/logout`;
	}

	let deleteAccountConfirmationModal = $state<Modal>();

	function initiateDeleteAccount() {
		deleteAccountConfirmationModal?.show();
	}

	async function deleteAccount() {
		await userService.deleteAccount();
		logout();
	}

	async function copyAccessToken() {
		const response = await loginService.token();
		if (response.type === "success" && response.data) {
			copyToClipboard(response.data);
		} else {
			chipToasts.error("Token konnte nicht abgerufen werden");
		}
	}
</script>

<svelte:head>
	<title>GitButler | Benutzer</title>
</svelte:head>

{#if !$user?.id}
	<div class="not-logged-in">
		<h3 class="text-18 text-bold">Es sieht so aus, als wärst du nicht angemeldet</h3>
		<p class="text-14 text-body clr-text-2">
			Bitte <a class="underline" href="/login">melde dich an</a>, um auf dein Profil zuzugreifen
		</p>
	</div>
{:else}
	<div class="profile__content">
		<div class="profile__fields">
			<ProfileHeader user={$user} {userService} />

			{#if recentProjects.current.length > 0}
				<Loading loadable={notificationSettings.current}>
					{#snippet children(notificationSettings)}
						<NotificationSettings {notificationSettings} {notificationSettingsService} />
					{/snippet}
				</Loading>

				<ExperimentalSettings />

				<Spacer />
			{/if}

			{#if $user}
				<CardGroup.Item standalone>
					{#snippet title()}
						Abmelden
					{/snippet}
					{#snippet caption()}
						Bereit für eine Pause? Klicke hier, um dich abzumelden und zu entspannen.
					{/snippet}
					{#snippet actions()}
						<Button kind="outline" icon="logout" onclick={logout}>Abmelden</Button>
					{/snippet}
				</CardGroup.Item>

				<CardGroup>
					<CardGroup.Item>
						{#snippet title()}
							Zugriffstoken
						{/snippet}
						{#snippet caption()}
							Dein Zugriffstoken wird verwendet, um die GitButler-Clients bei unserer API zu authentifizieren.
							<br />
							Bewahre es sicher auf und aktualisiere es regelmäßig für mehr Sicherheit.
						{/snippet}
					</CardGroup.Item>
					<CardGroup.Item alignment="center">
						{#snippet caption()}
							Kopiere dein Token zur Verwendung mit der Desktop-App oder der CLI.
						{/snippet}
						{#snippet actions()}
							<Button kind="outline" icon="copy" onclick={copyAccessToken}>Token kopieren</Button>
						{/snippet}
					</CardGroup.Item>
					<CardGroup.Item>
						{#snippet caption()}
							Aktualisiere dein Token, wenn du ungewöhnliche Aktivitäten bemerkst.
							<br />
							Dadurch wirst du überall abgemeldet, einschließlich der Desktop-App.
						{/snippet}
						{#snippet actions()}
							<Button kind="outline" icon="refresh" onclick={refreshAccessToken}
								>Token aktualisieren</Button
							>
						{/snippet}
					</CardGroup.Item>
				</CardGroup>

				<Spacer />

				<CardGroup>
					<CardGroup.Item>
						{#snippet title()}
							Gefahrenzone
						{/snippet}
					</CardGroup.Item>
					<CardGroup.Item>
						{#snippet caption()}
							Lösche dein Konto und alle Daten dauerhaft.
							<br />
							Diese Aktion kann nicht rückgängig gemacht werden.
						{/snippet}
						{#snippet actions()}
							<Button style="danger" onclick={initiateDeleteAccount}>Mein Konto löschen…</Button>
						{/snippet}
					</CardGroup.Item>
				</CardGroup>
			{/if}
		</div>

		<div class="profile__side">
			<div class="profile_mobile-separator">
				<Spacer />
			</div>

			<div class="download-app-banner">
				<div class="download-card">
					<div class="download-card__header">
						<img class="download-card__icon" src="/images/app-icon.svg" alt="" />

						<p class="text-12 text-body clr-text-2 text-balance">
							Hol dir die Desktop-App für Mac, Windows und Linux.
						</p>
					</div>

					<Button style="gray" wide onclick={() => window.open(downloadLink, "_blank")}>
						{downloadButtonText}
					</Button>

					<hr class="download-card__divider" />

					<p class="download-card__other-text text-12">
						Hol dir die App für
						<a href={linksJson.resources.downloads.url} target="_self" rel="noopener noreferrer">
							andere Plattformen
						</a>
						↗
					</p>
				</div>
			</div>

			{#if $user?.supporter}
				<SupporterCard />
			{/if}

			<div class="tips-section">
				<a
					class="tip-link"
					href={linksJson.resources.documentation.url}
					target="_blank"
					rel="noopener noreferrer"
				>
					<div class="tip-link__title">
						<Icon name="docs" color="var(--text-2)" />
						<h3 class="text-14 text-semibold">Erste Schritte</h3>
					</div>
					<p class="text-12 text-body clr-text-2">
						Entdecke umfassende Anleitungen und bewährte Vorgehensweisen.
					</p>

					<span class="tip-link__arrow-icon">↗</span>
				</a>
				<a
					class="tip-link"
					href={linksJson.social.discord.url}
					target="_blank"
					rel="noopener noreferrer"
				>
					<div class="tip-link__title">
						<Icon name="discord" color="var(--text-2)" />
						<h3 class="text-14 text-semibold">Werde Teil der Community</h3>
					</div>
					<p class="text-12 text-body clr-text-2">Tritt unserem Discord bei für Hilfe und Austausch.</p>

					<span class="tip-link__arrow-icon">↗</span>
				</a>
				<a class="tip-link" href={linksJson.other.support.url}>
					<div class="tip-link__title">
						<Icon name="chat" color="var(--text-2)" />
						<h3 class="text-14 text-semibold">Brauchst du Hilfe?</h3>
					</div>
					<p class="text-12 text-body clr-text-2">
						Erstelle ein Issue auf GitHub. Wir helfen dir gerne!
					</p>

					<span class="tip-link__arrow-icon">↗</span>
				</a>
			</div>
		</div>
	</div>
{/if}

<Modal bind:this={deleteAccountConfirmationModal} title="Kontolöschung bestätigen" width="small">
	<p class="text-13 text-body">
		Bist du sicher, dass du dein Konto löschen möchtest?
		<br />
		Diese Aktion ist <b>unumkehrbar</b> und entfernt alle deine Daten dauerhaft von unseren Servern.
	</p>
	{#snippet controls(close)}
		<div class="flex flex-row gap-8 justify-end">
			<Button style="pop" onclick={close}>Abbrechen</Button>
			<Button style="danger" icon="bin" kind="outline" onclick={deleteAccount}
				>Endgültig löschen</Button
			>
		</div>
	{/snippet}
</Modal>

<style lang="postcss">
	.not-logged-in {
		display: flex;
		row-gap: 10px;
		grid-column: full-start / full-end;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 100%;
		text-align: center;
	}

	.profile__content {
		display: grid;
		grid-template-columns: subgrid;
		row-gap: 16px;
		grid-column: full-start / full-end;
		align-self: flex-start;
	}

	.profile__fields {
		display: grid;
		row-gap: 16px;
		grid-column: narrow-start / -6;
	}

	.profile__side {
		display: flex;
		row-gap: 16px;
		grid-column: -6 / narrow-end;
		flex-direction: column;
		align-items: end;
	}

	.tips-section {
		display: flex;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
	}

	.tip-link {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 16px;
		gap: 6px;
		border-bottom: 1px solid var(--border-2);
		background-color: var(--bg-1);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:last-child {
			border-bottom: none;
		}

		&:hover {
			background-color: var(--hover-bg-1);

			& .tip-link__arrow-icon {
				transform: translateX(0);
				opacity: 1;
			}
		}
	}

	.tip-link__title {
		display: flex;
		align-items: center;
		margin-bottom: 4px;
		gap: 8px;
	}

	.tip-link__arrow-icon {
		position: absolute;
		top: 8px;
		right: 10px;
		transform: translateX(-2px);
		color: var(--text-2);
		font-size: 16px;
		opacity: 0;
		transition:
			opacity var(--transition-fast),
			color var(--transition-fast),
			transform var(--transition-medium);
	}

	.profile_mobile-separator {
		display: none;
		width: 100%;
	}

	.download-app-banner {
		width: 100%;
	}

	.download-card {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 16px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
		background-color: var(--bg-1);
	}

	.download-card__header {
		display: flex;
		gap: 12px;
	}

	.download-card__icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 38px;
		height: 38px;
		border-radius: var(--radius-m);
		background-color: var(--bg-pop);
	}

	.download-card__other-text {
		color: var(--text-2);
		text-align: center;
		transition: color var(--transition-fast);

		& a {
			text-decoration: underline;

			&:hover {
				color: var(--text-1);
				text-decoration: underline wavy var(--fill-pop-bg);
			}
		}
	}

	.download-card__divider {
		width: 100%;
		height: 1px;
		border: none;
		background: repeating-linear-gradient(
			to right,
			var(--border-2),
			var(--border-2) 2px,
			transparent 2px,
			transparent 4px
		);
	}

	@media (--tablet-viewport) {
		.profile__fields {
			grid-column: full-start / -5;
		}

		.profile__side {
			grid-column: -5 / full-end;
			align-items: center;
		}
	}

	@media (--mobile-viewport) {
		.profile__fields {
			grid-column: full-start / full-end;
		}

		.profile__side {
			grid-column: full-start / full-end;
			align-items: center;
		}

		.profile_mobile-separator {
			display: block;
		}

		.download-card {
			align-items: center;
		}

		.download-card__header {
			flex-direction: column;
			align-items: center;
			text-align: center;
		}
	}
</style>
