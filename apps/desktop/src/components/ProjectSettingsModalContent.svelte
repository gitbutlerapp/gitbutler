<script lang="ts">
	import CloudForm from '$components/CloudForm.svelte';
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import GitForm from '$components/GitForm.svelte';
	import PreferencesForm from '$components/PreferencesForm.svelte';
	import GeneralSettings from '$components/projectSettings/GeneralSettings.svelte';
	import { Button, Icon } from '@gitbutler/ui';
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { type Component } from 'svelte';
	import type { ProjectSettingsModalState } from '$lib/state/uiState.svelte';

	type Page = {
		id: string;
		label: string;
		icon: keyof typeof iconsJson;
		component: Component<{ projectId: string }>;
	};

	type Props = {
		data: ProjectSettingsModalState;
		close: () => void;
	};

	const { data, close }: Props = $props();

	const pages: Page[] = [
		{
			id: 'project',
			label: 'Project',
			icon: 'profile',
			component: GeneralSettings
		},
		{
			id: 'git',
			label: 'Git stuff',
			icon: 'git',
			component: GitForm
		},
		{
			id: 'ai',
			label: 'AI options',
			icon: 'ai',
			component: CloudForm
		},
		{
			id: 'experimental',
			label: 'Experimental',
			icon: 'idea',
			component: PreferencesForm
		}
	];

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
			<h3 class="text-16 text-bold">Project settings</h3>
		</div>
		<div class="settings-sidebar__links">
			{#each pages as page}
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
	</div>

	<section class="page-view" use:focusable={{ list: true }}>
		<ConfigurableScrollableContainer>
			<div class="page-view__content">
				{#if currentPage}
					<currentPage.component projectId={data.projectId} />
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
</style>
