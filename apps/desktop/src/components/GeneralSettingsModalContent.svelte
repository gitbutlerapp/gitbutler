<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import SupportersBanner from '$components/SupportersBanner.svelte';
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
	import { Button, Icon } from '@gitbutler/ui';
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { type Component } from 'svelte';
	import type { GeneralSettingsModalState } from '$lib/state/uiState.svelte';

	type Page = {
		id: string;
		label: string;
		icon: keyof typeof iconsJson;
		component: Component;
		adminOnly?: boolean;
	};

	type Props = {
		data: GeneralSettingsModalState;
		close: () => void;
	};

	const { data, close }: Props = $props();

	const userService = inject(USER_SERVICE);
	const user = userService.user;

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

	const urlService = inject(URL_SERVICE);

	let currentSelectedId = $state(data.selectedId || pages[0]!.id);
	const currentPage = $derived(pages.find((p) => p.id === currentSelectedId));

	function selectPage(pageId: string) {
		currentSelectedId = pageId;
	}
</script>

<div class="modal-settings-wrapper" use:focusable>
	<div class="settings-sidebar" use:focusable={{ list: true }}>
		<div class="settings-sidebar__title">
			<Button icon="chevron-left" kind="ghost" onclick={close} />
			<h3 class="text-16 text-bold">Global settings</h3>
		</div>
		<div class="settings-sidebar__links">
			{#each pages.filter((p) => !p.adminOnly || $user?.role === 'admin') as page}
				{@const selected = page.id === currentSelectedId}
				<button
					type="button"
					class="text-14 text-semibold settings-sidebar__links-item"
					class:selected
					onclick={() => selectPage(page.id)}
				>
					<div class="settings-sidebar__links-item__icon">
						<Icon name={page.icon} />
					</div>
					<span> {page.label}</span>
				</button>
			{/each}
		</div>

		<div class="settings-sidebar__footer">
			<SupportersBanner />

			<div class="social-banners">
				<button
					type="button"
					class="social-banner"
					onclick={async () =>
						await urlService.openExternalUrl(
							'mailto:hello@gitbutler.com?subject=Feedback or question!'
						)}
				>
					<span class="text-14 text-bold">Contact us</span>
					<Icon name="mail" />
				</button>
				<button
					type="button"
					class="social-banner"
					onclick={async () => await urlService.openExternalUrl('https://discord.gg/MmFkmaJ42D')}
				>
					<span class="text-14 text-bold">Join our Discord</span>
					<Icon name="discord" />
				</button>
			</div>
		</div>
	</div>

	<section class="page-view" use:focusable={{ list: true }}>
		<ConfigurableScrollableContainer>
			<div class="page-view__content">
				{#if currentPage}
					<h1 class="page-view__title text-head-20">
						{currentPage.label}
					</h1>
					<currentPage.component />
				{:else}
					Settings page {currentSelectedId} not Found.
				{/if}
			</div>
		</ConfigurableScrollableContainer>
	</section>
</div>

<style lang="postcss">
	.modal-settings-wrapper {
		display: flex;
		position: relative;
		width: 100%;
		height: 600px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.settings-sidebar {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 250px;
		padding: 16px 12px 12px 12px;
		border-right: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}

	.settings-sidebar__title {
		display: flex;
		align-items: center;

		& h3 {
			margin-left: 8px;
		}
	}

	/* LINKS */
	.settings-sidebar__links {
		display: flex;
		flex: 1;
		flex-direction: column;
		margin-top: 20px;
		gap: 2px;
	}

	.settings-sidebar__links-item {
		display: flex;
		position: relative;
		align-items: center;
		padding: 10px 8px;
		gap: 10px;
		border: none;
		border-radius: var(--radius-m);
		background: transparent;
		color: inherit;
		cursor: pointer;
		transition: background-color var(--transition-fast);

		&::after {
			position: absolute;
			top: 50%;
			left: -12px;
			width: 6px;
			height: 18px;
			transform: translateY(-50%) translateX(-100%);
			border-radius: 0 var(--radius-m) var(--radius-m) 0;
			background-color: var(--clr-selected-in-focus-element);
			content: '';
			transition:
				background-color var(--transition-fast),
				transform var(--transition-medium);
		}

		&.selected {
			background-color: var(--clr-bg-1-muted);

			& .settings-sidebar__links-item__icon {
				color: var(--clr-text-1);
			}

			&::after {
				transform: translateY(-50%) translateX(0);
			}
		}

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.settings-sidebar__links-item__icon {
		display: flex;
		color: var(--clr-text-3);
		transition: color var(--transition-fast);
	}

	.settings-sidebar__footer {
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
		color: var(--clr-text-2);
		transition: all var(--transition-fast);

		&:hover {
			border: 1px solid var(--clr-border-3);
			background-color: var(--clr-bg-1-muted);
		}
	}

	/* PAGE VIEW */
	.page-view {
		flex: 1;
		width: 100%;
		height: 100%;
		background-color: var(--clr-bg-2);
	}

	.page-view__content {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 640px;
		margin: 0 auto;
		padding: 24px 32px 32px;
		gap: 16px;
	}

	.page-view__title {
		align-self: flex-start;
		color: var(--clr-scale-ntrl-0);
	}
</style>
