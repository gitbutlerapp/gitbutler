<!--
	MultiDiffView - A virtualized multi-file diff viewer

	This component renders a scrollable list of file diffs for commits, branches, or worktree changes.
	It uses VirtualList to efficiently handle large changesets by only rendering diffs that are
	currently visible in the viewport, significantly reducing memory usage and improving performance.

	@component
-->
<script lang="ts">
	import ChangedFilesContextMenu from '$components/ChangedFilesContextMenu.svelte';
	import Drawer from '$components/Drawer.svelte';
	import FilePreviewPlaceholder from '$components/FilePreviewPlaceholder.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import UnifiedDiffView from '$components/UnifiedDiffView.svelte';
	import { isExecutableStatus, type TreeChange } from '$lib/hunks/change';
	import { DIFF_SERVICE } from '$lib/hunks/diffService.svelte';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { type SelectionId } from '$lib/selection/key';
	import { SETTINGS } from '$lib/settings/userSettings';
	import { computeChangeStatus } from '$lib/utils/fileStatus';
	import { inject } from '@gitbutler/core/context';
	import { Button, FileViewHeader, HunkDiffSkeleton, VirtualList } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		stackId?: string;
		changes: TreeChange[];
		draggable?: boolean;
		selectable: boolean;
		showBorder?: boolean;
		showRoundedEdges?: boolean;
		startIndex?: number;
		selectionId: SelectionId;
		onclose?: () => void;
	};

	let {
		projectId,
		stackId,
		changes,
		draggable,
		selectable,
		showBorder = true,
		showRoundedEdges = true,
		startIndex,
		selectionId,
		onclose
	}: Props = $props();

	const diffService = inject(DIFF_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const userSettings = inject(SETTINGS);

	const singleDiffView = $derived($userSettings.singleDiffView);

	// Track expanded/collapsed state for each diff by file path
	// This persists across re-renders (e.g., when VirtualList recycles items)
	const diffExpandedState = new Map<string, boolean>();

	let virtualList = $state<VirtualList<TreeChange>>();
	let scrollContainer = $state<HTMLElement | null>(null);
	let highlightedIndex = $state<number | undefined>(startIndex);
	let contextMenus = $state<Record<string, ReturnType<typeof ChangedFilesContextMenu>>>({});
	let headerTriggers = $state<Record<string, HTMLElement>>({});
	let buttonElements = $state<Record<string, HTMLElement>>({});
	let menuOpenStates = $state<Record<string, boolean>>({});

	export function jumpToIndex(index: number) {
		highlightedIndex = index;
		if (!singleDiffView) {
			virtualList?.jumpToIndex(index);
		}
	}
</script>

{#snippet changeItem(change: TreeChange, index?: number, highlight?: boolean)}
	{@const diffQuery = diffService.getDiff(projectId, change)}
	{@const diffData = diffQuery.response}
	{@const isExecutable = isExecutableStatus(change.status)}
	{@const patchData = diffData?.type === 'Patch' ? diffData.subject : null}
	{@const isCollapsed = diffExpandedState.get(change.path) ?? false}
	<Drawer
		noshrink
		stickyHeader={!singleDiffView}
		reserveSpaceOnStuck={!!onclose}
		closeButtonPlaceholder
		scrollRoot={scrollContainer}
		collapsable={!singleDiffView}
		defaultCollapsed={isCollapsed}
		highlighted={highlight && highlightedIndex === index}
		onclose={singleDiffView ? onclose : undefined}
		ontoggle={(collapsed) => {
			diffExpandedState.set(change.path, collapsed);
		}}
	>
		{#snippet header()}
			<div class="full-width" bind:this={headerTriggers[change.path]}>
				<FileViewHeader
					filePath={change.path}
					fileStatus={computeChangeStatus(change)}
					linesAdded={patchData?.linesAdded}
					linesRemoved={patchData?.linesRemoved}
					executable={isExecutable}
				/>
			</div>
		{/snippet}

		{#snippet actions()}
			<ChangedFilesContextMenu
				bind:this={contextMenus[change.path]}
				{projectId}
				{stackId}
				{selectionId}
				leftClickTrigger={buttonElements[change.path]}
				trigger={headerTriggers[change.path]}
				onopen={() => {
					menuOpenStates[change.path] = true;
				}}
				onclose={() => {
					menuOpenStates[change.path] = false;
				}}
			/>
			<Button
				bind:el={buttonElements[change.path]}
				kind="ghost"
				icon="kebab"
				size="tag"
				activated={menuOpenStates[change.path]}
				onclick={async () => {
					const contextMenu = contextMenus[change.path];
					const buttonEl = buttonElements[change.path];
					if (!contextMenu || !buttonEl) return;

					const changes = await idSelection.treeChanges(projectId, selectionId);
					if (idSelection.has(change.path, selectionId) && changes.length > 0) {
						contextMenu.open(buttonEl, { changes });
					} else {
						contextMenu.open(buttonEl, { changes: [change] });
					}
				}}
			/>
		{/snippet}

		<ReduxResult {projectId} hideLoading result={diffQuery.result}>
			{#snippet children(diff)}
				<UnifiedDiffView
					{projectId}
					{stackId}
					commitId={selectionId.type === 'commit' ? selectionId.commitId : undefined}
					{draggable}
					{change}
					{diff}
					{selectable}
					{selectionId}
					topPadding
				/>
			{/snippet}
			{#snippet loading()}
				<div class="loading">
					<HunkDiffSkeleton />
				</div>
			{/snippet}
		</ReduxResult>
	</Drawer>
{/snippet}

<div
	class="multi-diff-view"
	bind:this={scrollContainer}
	class:no-border={!showBorder}
	class:no-rounded={!showRoundedEdges}
>
	{#if onclose && !singleDiffView}
		<div class="floating-close">
			<Button kind="ghost" icon="cross" size="tag" onclick={onclose} />
		</div>
	{/if}

	{#if changes && changes.length > 0}
		{#if singleDiffView}
			{@const index = highlightedIndex ?? startIndex ?? 0}
			{@const change = changes[index]}
			{#if change}
				<div class="single-diff-view">
					{@render changeItem(change, index)}
				</div>
			{/if}
		{:else}
			<VirtualList
				bind:this={virtualList}
				{startIndex}
				grow
				items={changes}
				defaultHeight={102}
				visibility="scroll"
				renderDistance={100}
			>
				{#snippet template(change, index)}
					{@render changeItem(change, index, true)}
				{/snippet}
			</VirtualList>
		{/if}
	{:else}
		<FilePreviewPlaceholder />
	{/if}
</div>

<style>
	.multi-diff-view {
		display: flex;
		position: relative;
		flex-grow: 1;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);

		&.no-border {
			border: none;
		}

		&.no-rounded {
			border-radius: 0;
		}
	}

	.floating-close {
		display: flex;
		z-index: var(--z-lifted);
		position: absolute;
		top: 9px;
		right: 9px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-s);
	}

	.single-diff-view {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		width: 100%;
		overflow-y: auto;
	}

	.loading {
		height: 130px;
		padding: 12px;
	}
</style>
