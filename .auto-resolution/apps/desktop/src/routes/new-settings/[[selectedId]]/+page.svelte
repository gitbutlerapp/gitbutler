<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import SettingsPages, { type Page } from '$components/SettingsPages.svelte';
	import SupportersBanner from '$components/SupportersBanner.svelte';
	import AiSettings from '$components/profileSettings/AiSettings.svelte';
	import ExperimentalSettings from '$components/profileSettings/ExperimentalSettings.svelte';
	import GeneralSettings from '$components/profileSettings/GeneralSettings.svelte';
	import GitSettings from '$components/profileSettings/GitSettings.svelte';
	import IntegrationsSettings from '$components/profileSettings/IntegrationsSettings.svelte';
	import OrganisationSettings from '$components/profileSettings/OrganisationSettings.svelte';
	import TelemetrySettings from '$components/profileSettings/TelemetrySettings.svelte';
	import AppearanceSettings from '$components/projectSettings/AppearanceSettings.svelte';
	import { newSettingsPath } from '$lib/routes/routes.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { Icon } from '@gitbutler/ui';

	const pages: Page[] = [
		{
			type: 'global',
			id: 'general',
			label: 'General',
			icon: 'settings',
			component: GeneralSettings
		},
		{
			type: 'global',
			id: 'appearance',
			label: 'Appearance',
			icon: 'appearance',
			component: AppearanceSettings
		},
		{
			type: 'global',
			id: 'git',
			label: 'Git stuff',
			icon: 'git',
			component: GitSettings
		},
		{
			type: 'global',
			id: 'integrations',
			label: 'Integrations',
			icon: 'integrations',
			component: IntegrationsSettings
		},
		{
			type: 'global',
			id: 'ai',
			label: 'AI Options',
			icon: 'ai',
			component: AiSettings
		},
		{
			type: 'global',
			id: 'telemetry',
			label: 'Telemetry',
			icon: 'stat',
			component: TelemetrySettings
		},
		{
			type: 'global',
			id: 'experimental',
			label: 'Experimental',
			icon: 'idea',
			component: ExperimentalSettings
		},
		{
			type: 'global',
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
		goto('/');
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
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		color: var(--clr-scale-ntrl-30);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}
</style>
