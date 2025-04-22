<script lang="ts">
	import SupportersBanner from '$components/SupportersBanner.svelte';
	import SettingsPages, { type Page } from '$components/v3/SettingsPages.svelte';
	import AiSettings from '$components/v3/profileSettings/AiSettings.svelte';
	import ExperimentalSettings from '$components/v3/profileSettings/ExperimentalSettings.svelte';
	import GeneralSettings from '$components/v3/profileSettings/GeneralSettings.svelte';
	import GitSettings from '$components/v3/profileSettings/GitSettings.svelte';
	import IntegrationsSettings from '$components/v3/profileSettings/IntegrationsSettings.svelte';
	import OrganisationSettings from '$components/v3/profileSettings/OrganisationSettings.svelte';
	import TelemetrySettings from '$components/v3/profileSettings/TelemetrySettings.svelte';
	import AppearanceSettings from '$components/v3/projectSettings/AppearanceSettings.svelte';
	import { newSettingsPath } from '$lib/routes/routes.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const pages: Page[] = [
		{
			id: 'general',
			label: 'General',
			icon: 'settings',
			component: GeneralSettings
		},
		{
			id: 'appearance',
			label: 'Appearance',
			icon: 'appearance',
			component: AppearanceSettings
		},
		{
			id: 'git',
			label: 'Git stuff',
			icon: 'git',
			component: GitSettings
		},
		{
			id: 'integrations',
			label: 'Integrations',
			icon: 'integrations',
			component: IntegrationsSettings
		},
		{
			id: 'ai',
			label: 'AI Options',
			icon: 'ai',
			component: AiSettings
		},
		{
			id: 'telemetry',
			label: 'Telemetry',
			icon: 'stat',
			component: TelemetrySettings
		},
		{
			id: 'experimental',
			label: 'Experimental',
			icon: 'idea',
			component: ExperimentalSettings
		},
		{
			id: 'organizations',
			label: 'Organizations',
			icon: 'idea',
			component: OrganisationSettings,
			adminOnly: true
		}
	];

	const selectedId = $derived(page.params.selectedId);
</script>

<SettingsPages
	title="Global settings"
	{selectedId}
	{pages}
	pageUrl={(pageId) => newSettingsPath(pageId)}
	isFullPage
	onclose={() => {
		if (history.length > 0) {
			history.back();
		} else {
			goto('/');
		}
	}}
	>{#snippet footer()}
		<div class="profile-sidebar__footer">
			<SupportersBanner />

			<div class="social-banners">
				<button
					type="button"
					class="social-banner"
					onclick={async () =>
						await openExternalUrl('mailto:hello@gitbutler.com?subject=Feedback or question!')}
				>
					<span class="text-14 text-bold">Contact us</span>
					<Icon name="mail" />
				</button>
				<button
					type="button"
					class="social-banner"
					onclick={async () => await openExternalUrl('https://discord.gg/MmFkmaJ42D')}
				>
					<span class="text-14 text-bold">Join our Discord</span>
					<Icon name="discord" />
				</button>
			</div>
		</div>
	{/snippet}
</SettingsPages>

<style lang="postcss">
	.profile-sidebar__footer {
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	/* BANNERS */
	.social-banners {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.social-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 16px;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
		color: var(--clr-scale-ntrl-30);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}
</style>
