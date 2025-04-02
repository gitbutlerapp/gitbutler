<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import { intersectionObserver } from '@gitbutler/ui/utils/intersectionObserver';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		title?: string;
		stackId: string;
		splitView?: boolean;
		header?: Snippet;
		extraActions?: Snippet;
		children: Snippet;
		filesSplitView?: Snippet;
	};

	const {
		title,
		projectId,
		stackId,
		splitView,
		header,
		extraActions,
		children,
		filesSplitView
	}: Props = $props();

	const [uiState] = inject(UiState);

	const projectUiState = $derived(uiState.project(projectId));
	const stackUiState = $derived(uiState.stack(stackId));

	const drawerIsFullScreen = $derived(projectUiState.drawerFullScreen.get());
	const heightRmResult = $derived(uiState.global.drawerHeight.get());
	const heightRm = $derived(`min(${heightRmResult.current}rem, 80%)`);
	const height = $derived(drawerIsFullScreen.current ? '100%' : heightRm);

	const contentWidth = $derived(uiState.global.drawerSplitViewWidth.get());

	let drawerDiv = $state<HTMLDivElement>();
	let viewportEl = $state<HTMLElement>();

	function onToggleExpand() {
		projectUiState.drawerFullScreen.set(!drawerIsFullScreen.current);
	}

	export function onClose() {
		projectUiState.drawerPage.set(undefined);
		stackUiState.selection.set(undefined);
	}

	let isHeaderSticky = $state(false);
</script>

<div
	class="drawer"
	bind:this={drawerDiv}
	style:height
	use:focusable={{ id: 'commit', parentId: 'main' }}
>
	<div class="drawer-wrap">
		<div
			use:intersectionObserver={{
				callback: (entry) => {
					if (entry?.isIntersecting) {
						isHeaderSticky = false;
					} else {
						isHeaderSticky = true;
					}
				},
				options: {
					root: null,
					rootMargin: `-1px 0px 0px 0px`,
					threshold: 1
				}
			}}
			class="drawer-header"
			class:is-sticky={isHeaderSticky}
		>
			<div class="drawer-header__main">
				{#if title}
					<h3 class="text-15 text-bold">
						{title}
					</h3>
				{/if}
				{#if header}
					{@render header()}
				{/if}
			</div>

			<div class="drawer-header__actions">
				{#if extraActions}
					{@render extraActions()}
				{/if}
				<Button
					kind="ghost"
					icon={drawerIsFullScreen.current ? 'chevron-down' : 'chevron-up'}
					onclick={onToggleExpand}
				/>
				<Button kind="ghost" icon="cross" onclick={onClose} />
			</div>
		</div>

		{#if children}
			<div class="drawer__content-wrap" class:files-split-view={splitView}>
				<div
					class="drawer__content-scroll"
					style:width={splitView ? `${contentWidth.current}rem` : 'auto'}
					bind:this={viewportEl}
				>
					<ConfigurableScrollableContainer>
						<div class="drawer__content">
							{@render children()}
						</div>
					</ConfigurableScrollableContainer>

					{#if splitView}
						<Resizer
							viewport={viewportEl}
							direction="right"
							minWidth={16}
							imitateBorder
							onWidth={(value) => uiState.global.drawerSplitViewWidth.set(value)}
						/>
					{/if}
				</div>

				{#if splitView && filesSplitView}
					<div class="drawer__files-split-view">
						<ConfigurableScrollableContainer>
							<div>
								{@render filesSplitView()}
							</div>
						</ConfigurableScrollableContainer>
					</div>
				{/if}
			</div>
		{/if}

		{#if !drawerIsFullScreen.current}
			<Resizer
				direction="up"
				viewport={drawerDiv}
				minHeight={11}
				borderRadius="ml"
				onHeight={(value) => uiState.global.drawerHeight.set(value)}
			/>
		{/if}
	</div>
</div>

<style>
	.drawer {
		position: relative;
		overflow: hidden;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		flex-grow: 1;
		width: 100%;
		box-sizing: border-box;
		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
	}

	.drawer-wrap {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.drawer-header {
		position: sticky;
		top: -1px;
		z-index: var(--z-ground);
		display: flex;
		align-items: center;
		gap: 6px;
		justify-content: space-between;
		height: 42px;
		padding: 0 6px 0 14px;
		background-color: var(--clr-bg-2);
		border-bottom: 1px solid var(--clr-border-2);

		&.is-sticky {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.drawer-header__main {
		flex-grow: 1;
		display: flex;
		gap: 8px;
		overflow: hidden;
	}

	.drawer-header__actions {
		flex-shrink: 0;
		display: flex;
		gap: 2px;
	}

	.drawer__content-wrap {
		flex: 1;
		position: relative;
		display: flex;
		flex-direction: column;
		overflow: hidden;

		&.files-split-view {
			flex-direction: row;

			& .drawer__content {
				min-width: 300px;
				max-width: 500px;
			}
		}

		&:not(.files-split-view) {
			& .drawer__content {
				flex: 1;
			}
		}
	}

	.drawer__content-scroll {
		position: relative;
		height: 100%;
	}

	.drawer__files-split-view {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.drawer__content {
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 14px;
	}
</style>
