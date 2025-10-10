<script lang="ts">
	import ExperimentalSettings from './components/ExperimentalSettings.svelte';
	import NotificationSettings from './components/NotificationSettings.svelte';
	import ProfileHeader from './components/ProfileHeader.svelte';
	import SshKeysSection from './components/SshKeysSection.svelte';
	import SupporterCard from './components/SupporterCard.svelte';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import linksJson from '$lib/data/links.json';
	import { SSH_KEY_SERVICE } from '$lib/sshKeyService';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/core/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { getRecentlyPushedProjects } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import { NOTIFICATION_SETTINGS_SERVICE } from '@gitbutler/shared/settings/notificationSettingsService';
	import { getNotificationSettingsInterest } from '@gitbutler/shared/settings/notificationSetttingsPreview.svelte';
	import { Button, Icon, SectionCard, Spacer } from '@gitbutler/ui';
	import { env } from '$env/dynamic/public';

	const authService = inject(AUTH_SERVICE);
	const userService = inject(USER_SERVICE);
	const appState = inject(APP_STATE);
	const notificationSettingsService = inject(NOTIFICATION_SETTINGS_SERVICE);
	const sshKeyService = inject(SSH_KEY_SERVICE);
	const recentProjects = getRecentlyPushedProjects();

	const notificationSettings = getNotificationSettingsInterest(
		appState,
		notificationSettingsService
	);

	const user = $derived(userService.user);
	const token = $derived(authService.tokenReadable);

	// Detect user's operating system
	const detectedOS = $derived.by(() => {
		if (typeof window === 'undefined') return 'macOS';

		const userAgent = window.navigator.userAgent.toLowerCase();

		if (userAgent.includes('mac')) return 'macOS';
		if (userAgent.includes('win')) return 'Windows';
		if (userAgent.includes('linux')) return 'Linux';

		return 'macOS'; // default fallback
	});

	const downloadButtonText = $derived(`Download GitButler for ${detectedOS}`);

	const downloadLink = $derived.by(() => {
		switch (detectedOS) {
			case 'Windows':
				return linksJson.downloads.windowsMsi.url;
			case 'Linux':
				return linksJson.downloads.linuxAppimage.url;
			case 'macOS':
			default:
				return linksJson.downloads.appleSilicon.url;
		}
	});

	function logout() {
		authService.clearToken();
		window.location.href = `${env.PUBLIC_APP_HOST}cloud/logout`;
	}
</script>

<svelte:head>
	<title>GitButler | User</title>
</svelte:head>

{#if !$token || !$user?.id}
	<div class="not-logged-in">
		<h3 class="text-18 text-bold">It looks like you're not logged in</h3>
		<p class="text-14 text-body clr-text-2">
			Please <a class="underline" href="/login">log in</a> to access your profile
		</p>
	</div>
{:else}
	<div class="profile__content">
		<div class="profile__fields">
			<ProfileHeader user={$user} {userService} />

			{#if recentProjects.current.length > 0}
				<SshKeysSection {sshKeyService} {userService} />

				<Loading loadable={notificationSettings.current}>
					{#snippet children(notificationSettings)}
						<NotificationSettings {notificationSettings} {notificationSettingsService} />
					{/snippet}
				</Loading>

				<ExperimentalSettings />
			{/if}

			<Spacer />
			{#if $user}
				<SectionCard orientation="row">
					{#snippet title()}
						Signing out
					{/snippet}
					{#snippet caption()}
						Ready to take a break? Click here to log out and unwind.
					{/snippet}
					{#snippet actions()}
						<Button style="error" icon="signout" onclick={logout}>Log out</Button>
					{/snippet}
				</SectionCard>
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
							Get the desktop app for Mac, Windows, and Linux.
						</p>
					</div>

					<Button style="neutral" wide onclick={() => window.open(downloadLink, '_blank')}>
						{downloadButtonText}
					</Button>

					<hr class="download-card__divider" />

					<p class="download-card__other-text text-12">
						Get the app for
						<a href={linksJson.resources.downloads.url} target="_self" rel="noopener noreferrer">
							other platforms
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
						<Icon name="docs-small" color="var(--clr-text-2)" />
						<h3 class="text-14 text-semibold">Get Started</h3>
					</div>
					<p class="text-12 text-body clr-text-2">
						Explore comprehensive guides and best practices.
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
						<Icon name="discord-outline" color="var(--clr-text-2)" />
						<h3 class="text-14 text-semibold">Join the Community</h3>
					</div>
					<p class="text-12 text-body clr-text-2">Join our Discord for help and discussion.</p>

					<span class="tip-link__arrow-icon">↗</span>
				</a>
				<a class="tip-link" href={linksJson.other.support.url}>
					<div class="tip-link__title">
						<Icon name="chat" color="var(--clr-text-2)" />
						<h3 class="text-14 text-semibold">Need Help?</h3>
					</div>
					<p class="text-12 text-body clr-text-2">
						Create an issue on GitHub. We're here to assist!
					</p>

					<span class="tip-link__arrow-icon">↗</span>
				</a>
			</div>
		</div>
	</div>
{/if}

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
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.tip-link {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 16px;
		gap: 6px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:last-child {
			border-bottom: none;
		}

		&:hover {
			background-color: var(--clr-bg-1-muted);

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
		color: var(--clr-text-2);
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
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
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
		background-color: var(--clr-theme-pop-bg);
	}

	.download-card__other-text {
		color: var(--clr-text-2);
		text-align: center;
		transition: color var(--transition-fast);

		& a {
			text-decoration: underline;

			&:hover {
				color: var(--clr-text-1);
				text-decoration: underline wavy var(--clr-theme-pop-element);
			}
		}
	}

	.download-card__divider {
		width: 100%;
		height: 1px;
		border: none;
		background: repeating-linear-gradient(
			to right,
			var(--clr-border-2),
			var(--clr-border-2) 2px,
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
