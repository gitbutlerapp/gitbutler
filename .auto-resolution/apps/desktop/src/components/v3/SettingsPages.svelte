<script module>
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import { type Component } from 'svelte';
	import type { Snippet } from 'svelte';

	export type Page = {
		id: string;
		label: string;
		icon: keyof typeof iconsJson;
		component: Component;
		// Only show for admins.
		adminOnly?: boolean;
	};
</script>

<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { platformName } from '$lib/platform/platform';
	import { UserService } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	type Props = {
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
		title,
		selectedId: selectedId,
		pages,
		pageUrl,
		onclose,
		hidePageHeader,
		isFullPage,
		footer
	}: Props = $props();

	const [userService] = inject(UserService);

	const user = userService.user;
	const shownId = $derived(selectedId || pages[0]!.id);
	const shownPage = $derived(selectedId ? pages.find((p) => p.id === shownId) : pages[0]);
</script>

<div class="settings-wrap" class:full-page={isFullPage} class:chrome-page={!isFullPage}>
	{#if platformName === 'macos' && isFullPage}
		<div data-tauri-drag-region class="page-drag-bar"></div>
	{/if}
	<div class="settings-sidebar">
		<div class="settings-sidebar__title">
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
					<shownPage.component />
				{:else}
					Settings page {selectedId} not Found.
				{/if}
			</div>
		</ConfigurableScrollableContainer>
	</section>
</div>

<style lang="postcss">
	.settings-wrap {
		position: relative;
		display: flex;
		width: 100%;

		&.chrome-page {
			overflow: hidden;
			height: 100%;
			border-radius: var(--radius-ml);
			border: 1px solid var(--clr-border-2);
		}

		&.full-page {
			& .settings-sidebar__title {
				margin-top: 32px;
			}
		}
	}

	.settings-sidebar {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 250px;
		padding: 20px 12px 12px 12px;
		background-color: var(--clr-bg-1);
		border-right: 1px solid var(--clr-border-2);
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
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
		margin-top: 20px;
	}

	.settings-sidebar__links-item {
		position: relative;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 8px;
		border-radius: var(--radius-m);
		transition: background-color var(--transition-fast);

		&::after {
			content: '';
			position: absolute;
			top: 50%;
			left: -12px;
			width: 6px;
			height: 18px;
			transform: translateY(-50%) translateX(-100%);
			border-radius: 0 var(--radius-m) var(--radius-m) 0;
			background-color: var(--clr-selected-in-focus-element);
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
		user-select: none;
		width: 100%;
		height: 100%;
		flex: 1;
		background-color: var(--clr-bg-2);
	}

	.page-view__content {
		padding: 24px 32px 32px;
		display: flex;
		flex-direction: column;
		gap: 16px;
		max-width: 640px;
		width: 100%;
		margin: auto;
	}

	.page-view__title {
		color: var(--clr-scale-ntrl-0);
		align-self: flex-start;
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
