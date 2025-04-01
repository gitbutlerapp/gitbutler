<script lang="ts">
	import SettingsPages, { type Page } from '$components/v3/SettingsPages.svelte';
	import AiSettings from '$components/v3/profileSettings/AiSettings.svelte';
	import ExperimentalSettings from '$components/v3/profileSettings/ExperimentalSettings.svelte';
	import GitSettings from '$components/v3/profileSettings/GitSettings.svelte';
	import IntegrationSettings from '$components/v3/profileSettings/IntegrationsSettings.svelte';
	import ProfileSettings from '$components/v3/profileSettings/ProfileSettings.svelte';
	import TelemetrySettings from '$components/v3/profileSettings/TelemetrySettings.svelte';
	import AppearanceSettings from '$components/v3/projectSettings/AppearanceSettings.svelte';
	import { newSettingsPath } from '$lib/routes/routes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';

	const pages: Page[] = [
		{
			id: 'profile',
			label: 'Profile',
			icon: 'profile',
			component: ProfileSettings
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
			component: IntegrationSettings
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
		}
	];

	const selectedId = $derived(page.params.selectedId);
</script>

<SettingsPages
	title="Profile settings"
	{selectedId}
	{pages}
	pageUrl={(pageId) => newSettingsPath(pageId)}
>
	{#snippet close()}
		<Button
			icon="chevron-left"
			kind="ghost"
			onmousedown={() => {
				if (history.length > 0) {
					history.back();
				} else {
					goto('/');
				}
			}}
		/>
	{/snippet}
</SettingsPages>
