<script lang="ts">
	import SettingsModalLayout from '$components/SettingsModalLayout.svelte';

	import AiSettings from '$components/profileSettings/AiSettings.svelte';
	import ExperimentalSettings from '$components/profileSettings/ExperimentalSettings.svelte';
	import GeneralSettings from '$components/profileSettings/GeneralSettings.svelte';
	import GitSettings from '$components/profileSettings/GitSettings.svelte';
	import IntegrationsSettings from '$components/profileSettings/IntegrationsSettings.svelte';
	import OrganisationSettings from '$components/profileSettings/OrganisationSettings.svelte';
	import TelemetrySettings from '$components/profileSettings/TelemetrySettings.svelte';
	import AppearanceSettings from '$components/projectSettings/AppearanceSettings.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { URL_SERVICE } from '$lib/utils/url';
	import { inject } from '@gitbutler/core/context';
	import { Icon } from '@gitbutler/ui';
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import type { GeneralSettingsModalState } from '$lib/state/uiState.svelte';

	type Props = {
		data: GeneralSettingsModalState;
	};

	const { data }: Props = $props();

	const userService = inject(USER_SERVICE);
	const user = userService.user;
	const urlService = inject(URL_SERVICE);

	const pages = [
		{
			id: 'general',
			label: 'General',
			icon: 'settings' as keyof typeof iconsJson
		},
		{
			id: 'appearance',
			label: 'Appearance',
			icon: 'appearance' as keyof typeof iconsJson
		},
		{
			id: 'git',
			label: 'Git stuff',
			icon: 'git' as keyof typeof iconsJson
		},
		{
			id: 'integrations',
			label: 'Integrations',
			icon: 'integrations' as keyof typeof iconsJson
		},
		{
			id: 'ai',
			label: 'AI Options',
			icon: 'ai' as keyof typeof iconsJson
		},
		{
			id: 'telemetry',
			label: 'Telemetry',
			icon: 'stat' as keyof typeof iconsJson
		},
		{
			id: 'experimental',
			label: 'Experimental',
			icon: 'idea' as keyof typeof iconsJson
		},
		{
			id: 'organizations',
			label: 'Organizations',
			icon: 'idea' as keyof typeof iconsJson,
			adminOnly: true
		}
	];

	let currentSelectedId = $state(data.selectedId || pages[0]!.id);

	function selectPage(pageId: string) {
		currentSelectedId = pageId;
	}
</script>

<SettingsModalLayout
	title="Global settings"
	{pages}
	selectedId={data.selectedId}
	isAdmin={$user?.role === 'admin'}
	onSelectPage={selectPage}
>
	{#snippet content({ currentPage })}
		{#if currentPage}
			{#if currentPage.id === 'general'}
				<GeneralSettings />
			{:else if currentPage.id === 'appearance'}
				<AppearanceSettings />
			{:else if currentPage.id === 'git'}
				<GitSettings />
			{:else if currentPage.id === 'integrations'}
				<IntegrationsSettings />
			{:else if currentPage.id === 'ai'}
				<AiSettings />
			{:else if currentPage.id === 'telemetry'}
				<TelemetrySettings />
			{:else if currentPage.id === 'experimental'}
				<ExperimentalSettings />
			{:else if currentPage.id === 'organizations'}
				<OrganisationSettings />
			{:else}
				Settings page {currentPage.id} not Found.
			{/if}
		{:else}
			Settings page {currentSelectedId} not Found.
		{/if}
	{/snippet}

	{#snippet footer()}
		<div class="social">
			<button
				type="button"
				class="social-btn"
				onclick={async () => await urlService.openExternalUrl('https://docs.gitbutler.com/')}
			>
				<Icon name="docs" />
				<span class="text-13 text-bold full-width">Docs</span>
				<div class="open-link-icon">
					<Icon name="open-link" />
				</div>
			</button>
			<button
				type="button"
				class="social-btn"
				onclick={async () => await urlService.openExternalUrl('https://discord.gg/MmFkmaJ42D')}
			>
				<Icon name="discord" />
				<span class="text-13 text-bold full-width">Our Discord</span>
				<div class="open-link-icon">
					<Icon name="open-link" />
				</div>
			</button>
		</div>
	{/snippet}
</SettingsModalLayout>

<style lang="postcss">
	/* BANNERS */
	.social {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.social-btn {
		display: flex;
		align-items: center;
		padding: 8px 12px;
		gap: 12px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		color: var(--clr-text-2);
		text-align: left;
		transition: all var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);

			.open-link-icon {
				display: flex;
			}
		}
	}

	.open-link-icon {
		display: none;
	}
</style>
