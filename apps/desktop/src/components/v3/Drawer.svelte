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
		noLeftPadding?: boolean;
		header?: Snippet;
		extraActions?: Snippet;
		kebabMenu?: Snippet<[element: HTMLElement]>;
		children: Snippet;
		filesSplitView?: Snippet;
		disableScroll?: boolean;
		testId?: string;
	};

	const {
		title,
		projectId,
		stackId,
		minHeight = 11,
		noLeftPadding,
		header,
		extraActions,
		kebabMenu,
		children,
		filesSplitView,
		disableScroll,
		testId
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

	let headerDiv = $state<HTMLDivElement>();
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
	data-testid={testId}
	class="drawer"
	bind:this={drawerDiv}
	style:height
	style:min-height="{minHeight}rem"
	use:focusable={{ id: Focusable.Drawer, parentId: Focusable.WorkspaceMiddle }}
>
	<div class="drawer-wrap">
		<div bind:this={headerDiv} class="drawer-header" class:no-left-padding={noLeftPadding}>
			<div class="drawer-header__title">
				{#if title}
					<h3 class="text-15 text-bold truncate">
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
						{@render kebabMenu(headerDiv)}
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
	</div>

	{#if !drawerIsFullScreen.current}
		<!-- Resizer should be outside if the overflow: hidden container otherwise it wouldn't overlay on top of the border -->
		<Resizer
			direction="up"
			viewport={drawerDiv}
			{minHeight}
			borderRadius="ml"
			onHeight={(value) => uiState.global.drawerHeight.set(value)}
		/>
	{/if}
</div>

<style>
	.drawer {
		box-sizing: border-box;
		container-name: drawer;

		container-type: inline-size;
		display: flex;
		position: relative;
		flex-grow: 1;
		flex-shrink: 0;
		flex-direction: column;
		width: 100%;
	}

	.drawer-wrap {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
	}

	.drawer-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 42px;
		padding: 0 8px 0 14px;
		gap: 6px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.drawer-header__title {
		display: flex;
		flex-grow: 1;
		align-items: center;
		height: 100%;
		overflow: hidden;
		gap: 8px;
	}

	.drawer-header__actions {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		gap: 12px;
	}

	.drawer-header__actions-group {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.drawer__content-resizer {
		/* need this to hide the resizer on smaller screens */
		display: contents;
	}

	.drawer__content-wrap {
		display: flex;
		position: relative;
		flex: 1;
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

		@container drawer (min-width: 530px) {
			&.files-split-view .drawer__content-scroll {
				max-width: 500px;
			}
		}

		@container drawer (max-width: 530px) {
			&.files-split-view {
				flex-direction: column;
			}

			& .drawer__content-scroll {
				width: 100%;
				height: auto;
			}

			& .drawer__content-resizer {
				display: none;
			}

			& .drawer__files-split-view {
				border-top: 1px solid var(--clr-border-2);
			}
		}
	}

	.drawer__content {
		container-name: drawer-content;
		container-type: inline-size;
		display: flex;
		position: relative;
		flex-direction: column;
		min-height: 100%;
		padding: 14px;
	}

	.drawer__content-scroll {
		display: flex;
		position: relative;
		flex-direction: column;
		width: var(--custom-width);
		height: 100%;
		min-height: 0;
	}

	.drawer__files-split-view {
		display: flex;
		flex: 1;
		flex-direction: column;
		min-width: 200px;
		overflow: hidden;
	}

	/* MODIFIERS */
	.drawer-header {
		&.no-left-padding {
			padding-left: 0;
		}
	}
</style>
