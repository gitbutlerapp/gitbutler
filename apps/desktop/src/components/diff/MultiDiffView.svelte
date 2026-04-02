<!--
	MultiDiffView - A virtualized multi-file diff viewer

	This component renders a scrollable list of file diffs for commits, branches, or worktree changes.
	It uses VirtualList to efficiently handle large changesets by only rendering diffs that are
	currently visible in the viewport, significantly reducing memory usage and improving performance.

	@component
-->
<script lang="ts">
	import FilePreviewPlaceholder from "$components/diff/FilePreviewPlaceholder.svelte";
	import FloatingDiffModal from "$components/diff/FloatingDiffModal.svelte";
	import UnifiedDiffView from "$components/diff/UnifiedDiffView.svelte";
	import ChangedFilesContextMenu from "$components/shared/ChangedFilesContextMenu.svelte";
	import Drawer from "$components/shared/Drawer.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import { computeChangeStatus } from "$lib/files/fileStatus";
	import { isExecutableStatus, type TreeChange } from "$lib/hunks/change";
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { type SelectionId } from "$lib/selection/key";
	import { ScrollSelectionLock } from "$lib/selection/scrollSelectionLock.svelte";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { inject } from "@gitbutler/core/context";
	import { Button, FileViewHeader, HunkDiffSkeleton, VirtualList } from "@gitbutler/ui";
	import { untrack } from "svelte";

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
		onVisibleChange?: (change: { start: number; end: number } | undefined) => void;
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
		onclose,
		onVisibleChange,
	}: Props = $props();

	const diffService = inject(DIFF_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const userSettings = inject(SETTINGS);

	const allInOneDiff = $derived($userSettings.allInOneDiff);
	const highlightDiffs = $derived($userSettings.highlightDiffs);

	// Not reactive by design — feeds `defaultCollapsed` as an initial value only,
	// so Svelte does not need to track mutations. Persists across VirtualList recycles.
	const diffExpandedState = new Map<string, boolean>();

	function getInitialLockedIndex() {
		if (startIndex === undefined) return undefined;
		return idSelection.collectionSize(selectionId) > 0 ? startIndex : undefined;
	}

	let virtualList = $state<VirtualList<TreeChange>>();
	let highlightedIndex = $state<number | undefined>(untrack(() => startIndex));
	// Prevents VirtualList scroll events from overwriting a user-clicked selection.
	const scrollLock = new ScrollSelectionLock(getInitialLockedIndex());
	let floatingDiffOpen = $state(false);
	let floatingDiffInitialIndex = $state(0);
	let contextMenu = $state<ChangedFilesContextMenu>();
	let activeMenuPath = $state<string | undefined>();

	export function jumpToIndex(index: number) {
		highlightedIndex = index;
		scrollLock.set(index);
		if (allInOneDiff) {
			virtualList?.jumpToIndex(index);
		}
	}

	export function openFloatingDiff() {
		floatingDiffInitialIndex = highlightedIndex ?? startIndex ?? 0;
		floatingDiffOpen = true;
	}
</script>

{#snippet popOutSnippet()}
	<Button
		kind="ghost"
		icon="pop-out-bottom-right"
		size="tag"
		tooltip="Pop out diff view"
		onclick={() => {
			floatingDiffInitialIndex = highlightedIndex ?? startIndex ?? 0;
			floatingDiffOpen = true;
		}}
	/>
{/snippet}

{#snippet changeItem(change: TreeChange, index?: number, highlight?: boolean)}
	{@const diffQuery = diffService.getDiff(projectId, change)}
	{@const diffData = diffQuery.response}
	{@const isExecutable = isExecutableStatus(change.status)}
	{@const patchData = diffData?.type === "Patch" ? diffData.subject : null}
	{@const isCollapsed = diffExpandedState.get(change.path) ?? false}
	<Drawer
		noshrink
		stickyHeader={allInOneDiff}
		reserveSpaceOnStuck={!!onclose}
		closeButtonPlaceholder={!!onclose}
		collapsable={allInOneDiff}
		defaultCollapsed={isCollapsed}
		highlighted={allInOneDiff && highlightDiffs && highlight && highlightedIndex === index}
		onclose={!allInOneDiff ? onclose : undefined}
		ontoggle={(collapsed) => {
			diffExpandedState.set(change.path, collapsed);
		}}
		closeActions={!allInOneDiff && onclose ? popOutSnippet : undefined}
	>
		{#snippet header()}
			<div class="full-width">
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
			<Button
				kind="ghost"
				icon="kebab"
				size="tag"
				activated={activeMenuPath === change.path}
				onclick={async (e) => {
					if (!contextMenu || !(e.target instanceof HTMLElement)) return;
					if (activeMenuPath === change.path) {
						contextMenu.close();
						return;
					}
					const changes = await idSelection.treeChanges(projectId, selectionId);
					if (idSelection.has(change.path, selectionId) && changes.length > 0) {
						contextMenu.open(e.target, { changes });
					} else {
						contextMenu.open(e.target, { changes: [change] });
					}
					activeMenuPath = change.path;
				}}
			/>
		{/snippet}

		<ReduxResult {projectId} hideLoading result={diffQuery.result}>
			{#snippet children(diff)}
				<UnifiedDiffView
					{projectId}
					{stackId}
					commitId={selectionId.type === "commit" ? selectionId.commitId : undefined}
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

<div class="multi-diff-view" class:no-border={!showBorder} class:no-rounded={!showRoundedEdges}>
	{#if onclose && allInOneDiff}
		<div class="floating-actions">
			<Button
				kind="ghost"
				icon="pop-out-bottom-right"
				size="tag"
				tooltip="Pop out diff view"
				onclick={openFloatingDiff}
			/>
			<Button kind="ghost" icon="cross" size="tag" onclick={onclose} />
		</div>
	{/if}

	{#if changes && changes.length > 0}
		<ChangedFilesContextMenu
			bind:this={contextMenu}
			{projectId}
			{stackId}
			{selectionId}
			onclose={() => {
				activeMenuPath = undefined;
			}}
		/>
		{#if !allInOneDiff}
			{@const index = highlightedIndex ?? startIndex ?? 0}
			{@const change = changes[index]}
			{#if change}
				<div class="single-diff-view" data-remove-from-panning>
					{@render changeItem(change, index)}
				</div>
			{/if}
		{:else}
			<VirtualList
				bind:this={virtualList}
				{startIndex}
				grow
				items={changes}
				defaultHeight={173}
				visibility="scroll"
				renderDistance={100}
				initSettleMs={500}
				onVisibleChange={(range) => {
					if (range) {
						const activeIndex = scrollLock.resolve(range);

						highlightedIndex = activeIndex;
						const activeChange = changes[activeIndex];
						const selectionSize = idSelection.collectionSize(selectionId);
						const shouldFollowScrollSelection = selectionSize <= 1;
						if (
							activeChange &&
							shouldFollowScrollSelection &&
							!idSelection.has(activeChange.path, selectionId)
						) {
							idSelection.set(activeChange.path, selectionId, activeIndex);
						}
					}
					onVisibleChange?.(range);
				}}
				getId={(change) => change.path}
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

{#if floatingDiffOpen}
	<FloatingDiffModal
		{projectId}
		{stackId}
		{changes}
		{selectionId}
		{draggable}
		{selectable}
		initialIndex={floatingDiffInitialIndex}
		onclose={() => {
			floatingDiffOpen = false;
		}}
	/>
{/if}

<style>
	.multi-diff-view {
		display: flex;
		position: relative;
		flex-grow: 1;
		width: 100%;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
		background-color: var(--bg-1);

		&.no-border {
			border: none;
		}

		&.no-rounded {
			border-radius: 0;
		}
	}

	.floating-actions {
		display: flex;
		z-index: var(--z-lifted);
		position: absolute;
		top: 6px;
		right: 6px;
		padding: 2px;
		gap: 2px;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-m);
		background-color: var(--bg-1);
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
