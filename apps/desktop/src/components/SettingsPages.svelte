<script lang="ts" module>
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import { type Component } from 'svelte';
	import type { Snippet } from 'svelte';

	export type Page = {
		id: string;
		label: string;
		icon: keyof typeof iconsJson;
		// Only show for admins.
		adminOnly?: boolean;
	} & (
		| {
				type: 'global';
				component: Component;
		  }
		| {
				type: 'project';
				component: Component<{ projectId: string }>;
		  }
	);
</script>

<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { platformName } from '$lib/platform/platform';
	import { USER_SERVICE } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	type Props = {
		projectId?: string;
		title: string;
		selectedId?: string;
		pages: Page[];
		pageUrl: (pageId: string) => string;
		onclose?: () => void;
		// If true, the page will be shown in full screen mode.
		hidePageHeader?: boolean;
		isFullPage?: boolean;
		footer?: Snippet;
	};

	const {
		projectId,
		title,
		selectedId: selectedId,
		pages,
		pageUrl,
		onclose,
		hidePageHeader,
		isFullPage,
		footer
	}: Props = $props();

	const userService = inject(USER_SERVICE);

	const user = userService.user;
	const shownId = $derived(selectedId || pages[0]!.id);
	const shownPage = $derived(selectedId ? pages.find((p) => p.id === shownId) : pages[0]);

	const isWithExtraSpace = $derived(platformName === 'macos' && isFullPage);
</script>

<div class="settings-wrap" class:chrome-page={!isFullPage}>
	{#if isWithExtraSpace}
		<div data-tauri-drag-region class="page-drag-bar"></div>
	{/if}
	<div class="settings-sidebar">
		<div class="settings-sidebar__title" class:top-margin={isWithExtraSpace}>
			{#if onclose}
				<Button icon="chevron-left" kind="ghost" onclick={onclose} />
			{/if}
			<h3 class="text-16 text-bold">{title}</h3>
		</div>
		<div class="settings-sidebar__links">
			{#each pages.filter((p) => !p.adminOnly || $user?.role === 'admin') as page}
				{@const selected = page.id === shownId}
				<a
					class="text-14 text-semibold settings-sidebar__links-item"
					class:selected
					href={pageUrl(page.id)}
					data-sveltekit-replacestate
				>
					<div class="settings-sidebar__links-item__icon">
						<Icon name={page.icon} />
					</div>
					<span> {page.label}</span>
				</a>
			{/each}
		</div>

		{#if footer}
			<div class="settings-sidebar__footer">
				{@render footer()}
			</div>
		{/if}
	</div>

	<section class="page-view">
		<ConfigurableScrollableContainer>
			<div class="page-view__content">
				{#if shownPage}
					{#if !hidePageHeader}
						<h1 class="page-view__title text-head-20">
							{shownPage.label}
						</h1>
					{/if}
					{#if projectId && shownPage.type === 'project'}
						<shownPage.component {projectId} />
					{:else if shownPage.type === 'global'}
						<shownPage.component />
					{/if}
				{:else}
					Settings page {selectedId} not Found.
				{/if}
			</div>
		</ConfigurableScrollableContainer>
	</section>
</div>

<style lang="postcss">
	.settings-wrap {
		display: flex;
		position: relative;
		width: 100%;

		&.chrome-page {
			height: 100%;
			overflow: hidden;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-ml);
		}
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

		&.top-margin {
			margin-top: 32px;
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
		border-radius: var(--radius-m);
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

	.page-view__title {
		align-self: flex-start;
		color: var(--clr-scale-ntrl-0);
	}

	/* OTHER */
	.page-drag-bar {
		z-index: var(--z-ground);
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 34px;
	}
</style>
