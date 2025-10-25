<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { Icon } from '@gitbutler/ui';
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { type Snippet } from 'svelte';

	type Page = {
		id: string;
		label: string;
		icon: keyof typeof iconsJson;
		adminOnly?: boolean;
		[key: string]: any; // Allow additional properties for flexibility
	};

	type Props = {
		title: string;
		pages: Page[];
		selectedId?: string;
		isAdmin?: boolean;
		onSelectPage: (pageId: string) => void;
		content: Snippet<[{ currentPage: Page | undefined }]>;
		footer?: Snippet;
	};

	const { title, pages, selectedId, isAdmin, onSelectPage, content, footer }: Props = $props();

	let currentSelectedId = $state(selectedId || pages[0]?.id || '');
	const currentPage = $derived(pages.find((p) => p.id === currentSelectedId));

	function selectPage(pageId: string) {
		currentSelectedId = pageId;
		onSelectPage(pageId);
	}
</script>

<div class="modal-settings-wrapper">
	<div class="settings-sidebar__wrapper">
		<div class="settings-sidebar" use:focusable>
			<h3 class="settings-sidebar__title text-16 text-bold">{title}</h3>
			<div class="settings-sidebar__links">
				{#each pages.filter((p) => !p.adminOnly || isAdmin) as page}
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
						<span>{page.label}</span>
					</button>
				{/each}
			</div>

			{#if footer}
				<div class="settings-sidebar__footer">
					{@render footer()}
				</div>
			{/if}
		</div>
	</div>

	<section class="page-view" use:focusable={{ vertical: true }}>
		<ConfigurableScrollableContainer>
			<div class="page-view__content">
				{@render content({ currentPage })}
			</div>
		</ConfigurableScrollableContainer>
	</section>
</div>

<style lang="postcss">
	.modal-settings-wrapper {
		display: flex;
		position: relative;
		width: 100%;
		height: 76vh;
		max-height: 1000px;
	}

	.settings-sidebar__wrapper {
		flex: 0 0 230px;
		padding: 8px;
		padding-right: 0;
		background-color: var(--clr-bg-2);
	}

	.settings-sidebar {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		padding: 16px 12px 12px 12px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
	}

	.settings-sidebar__title {
		padding: 4px 0 0 8px;
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
			background-color: var(--clr-bg-2);

			& .settings-sidebar__links-item__icon {
				color: var(--clr-text-1);
			}

			&::after {
				transform: translateY(-50%) translateX(0);
			}
		}

		&:not(.selected):hover {
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

	/* PAGE VIEW */
	.page-view {
		flex: 1;
		width: 100%;
		height: 100%;
		overflow: hidden;
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
