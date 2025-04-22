<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { Focusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		title?: string;
		stackId?: string;
		minHeight?: number;
		header?: Snippet;
		extraActions?: Snippet;
		kebabMenu?: Snippet;
		children: Snippet;
		filesSplitView?: Snippet;
		disableScroll?: boolean;
	};

	const {
		title,
		projectId,
		stackId,
		minHeight = 11,
		header,
		extraActions,
		kebabMenu,
		children,
		filesSplitView,
		disableScroll
	}: Props = $props();

	const [uiState] = inject(UiState);

	const projectUiState = $derived(uiState.project(projectId));
	const stackUiState = $derived(stackId ? uiState.stack(stackId) : undefined);

	const drawerIsFullScreen = $derived(projectUiState.drawerFullScreen.get());
	const heightRmResult = $derived(uiState.global.drawerHeight.get());
	const heightRm = $derived(`min(${heightRmResult.current}rem, 80%)`);
	const height = $derived(drawerIsFullScreen.current ? '100%' : heightRm);
	const splitView = $derived(!!filesSplitView);

	const contentWidth = $derived(uiState.global.drawerSplitViewWidth.get());
	const scrollable = $derived(!disableScroll);

	let drawerDiv = $state<HTMLDivElement>();
	let viewportEl = $state<HTMLElement>();

	function onToggleExpand() {
		projectUiState.drawerFullScreen.set(!drawerIsFullScreen.current);
	}

	export function onClose() {
		projectUiState.drawerPage.set(undefined);
		stackUiState?.selection.set(undefined);
	}
</script>

<div
	class="drawer"
	bind:this={drawerDiv}
	style:height
	style:min-height="{minHeight}rem"
	use:focusable={{ id: Focusable.CommitEditor, parentId: Focusable.WorkspaceMiddle }}
>
	<div class="drawer-wrap">
		<div class="drawer-header">
			<div class="drawer-header__title">
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
					<div class="drawer-header__actions-group">
						{@render extraActions()}
					</div>
				{/if}
				<div class="drawer-header__actions-group">
					{#if kebabMenu}
						{@render kebabMenu()}
					{/if}
					<Button
						kind="ghost"
						icon={drawerIsFullScreen.current ? 'chevron-down' : 'chevron-up'}
						size="tag"
						onclick={onToggleExpand}
					/>
					<Button kind="ghost" icon="cross" size="tag" onclick={onClose} />
				</div>
			</div>
		</div>

		{#if children}
			<div
				class="drawer__content-wrap"
				class:files-split-view={splitView}
				style:--custom-width={splitView ? `${contentWidth.current}rem` : 'auto'}
			>
				<div class="drawer__content-scroll" bind:this={viewportEl}>
					{#if scrollable}
						<ConfigurableScrollableContainer>
							<div class="drawer__content">
								{@render children()}
							</div>
						</ConfigurableScrollableContainer>
					{:else}
						<div class="drawer__content">
							{@render children()}
						</div>
					{/if}

					{#if splitView}
						<div class="drawer__content-resizer">
							<Resizer
								viewport={viewportEl}
								direction="right"
								minWidth={16}
								imitateBorder
								onWidth={(value) => uiState.global.drawerSplitViewWidth.set(value)}
							/>
						</div>
					{/if}
				</div>

				{#if splitView && filesSplitView}
					<div class="drawer__files-split-view">
						<ConfigurableScrollableContainer>
							{@render filesSplitView()}
						</ConfigurableScrollableContainer>
					</div>
				{/if}
			</div>
		{/if}

		{#if !drawerIsFullScreen.current}
			<Resizer
				direction="up"
				viewport={drawerDiv}
				{minHeight}
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
		container-type: inline-size;
		container-name: drawer;
	}

	.drawer-wrap {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.drawer-header {
		display: flex;
		align-items: center;
		gap: 6px;
		justify-content: space-between;
		height: 42px;
		padding: 0 8px 0 14px;
		background-color: var(--clr-bg-2);
		border-bottom: 1px solid var(--clr-border-2);
	}

	.drawer-header__title {
		height: 100%;
		flex-grow: 1;
		display: flex;
		gap: 8px;
		align-items: center;
		overflow: hidden;
	}

	.drawer-header__actions {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.drawer-header__actions-group {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.drawer__content-resizer {
		display: contents;
	}

	.drawer__content-wrap {
		flex: 1;
		position: relative;
		display: flex;
		flex-direction: column;
		overflow: hidden;

		&.files-split-view {
			flex-direction: row;
		}

		&:not(.files-split-view) {
			& .drawer__content {
				flex: 1;
			}

			& .drawer__content-scroll {
				height: 100%;
			}
		}

		&.files-split-view .drawer__content-scroll {
			min-width: 300px;
			max-width: 500px;
		}
	}

	.drawer__content-scroll {
		display: flex;
		flex-direction: column;
		position: relative;
		height: 100%;
		width: var(--custom-width);
		min-height: 0;
	}

	.drawer__files-split-view {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-width: 240px;
	}

	.drawer__content {
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 14px;
		min-height: 100%;
	}
</style>
