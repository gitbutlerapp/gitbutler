<script lang="ts">
	import ChangedFileStats from "$components/ChangedFileStats.svelte";
	import ChangedFilesContextMenu from "$components/ChangedFilesContextMenu.svelte";
	import Drawer from "$components/Drawer.svelte";
	import FileTreeList from "$components/FileTreeList.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import Resizer from "$components/Resizer.svelte";
	import UnifiedDiffView from "$components/UnifiedDiffView.svelte";
	import FloatingModal from "$lib/floating/FloatingModal.svelte";
	import { isExecutableStatus, type TreeChange } from "$lib/hunks/change";
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { type SelectionId } from "$lib/selection/key";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { computeChangeStatus } from "$lib/utils/fileStatus";
	import { inject } from "@gitbutler/core/context";
	import { Button, FileViewHeader, HunkDiffSkeleton, Icon, VirtualList } from "@gitbutler/ui";

	interface Props {
		projectId: string;
		stackId?: string;
		changes: TreeChange[];
		selectionId: SelectionId;
		draggable?: boolean;
		selectable?: boolean;
		initialIndex?: number;
		onclose: () => void;
	}

	const {
		projectId,
		stackId,
		changes,
		selectionId,
		draggable,
		selectable = false,
		initialIndex = 0,
		onclose,
	}: Props = $props();

	const diffService = inject(DIFF_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const userSettings = inject(SETTINGS);

	const singleDiffView = $derived($userSettings.singleDiffView);

	// Track expanded/collapsed state for each diff (for virtual list mode)
	const diffExpandedState = new Map<string, boolean>();

	let listMode: "list" | "tree" = $state("list");
	// eslint-disable-next-line svelte/prefer-writable-derived
	let selectedIndex = $state(0);
	$effect(() => {
		selectedIndex = initialIndex;
	});
	let headerElRef = $state<HTMLDivElement | undefined>(undefined);
	let leftPanelEl = $state<HTMLDivElement | undefined>(undefined);
	let virtualList = $state<VirtualList<TreeChange>>();
	let diffScrollContainer = $state<HTMLElement | null>(null);
	let fileListEl = $state<HTMLDivElement>();
	let fileListContextMenu = $state<ReturnType<typeof ChangedFilesContextMenu>>();
	let visibleRange = $state<{ start: number; end: number } | undefined>(undefined);
	let contextMenus = $state<Record<string, ReturnType<typeof ChangedFilesContextMenu>>>({});
	let headerTriggers = $state<Record<string, HTMLElement>>({});
	let buttonElements = $state<Record<string, HTMLElement>>({});
	let menuOpenStates = $state<Record<string, boolean>>({});

	const selectedChange = $derived(changes[selectedIndex]);

	const totalLinesAdded = $derived(
		changes.reduce((sum, change) => {
			const diff = diffService.getDiff(projectId, change).response;
			return sum + (diff?.type === "Patch" ? diff.subject.linesAdded : 0);
		}, 0),
	);
	const totalLinesRemoved = $derived(
		changes.reduce((sum, change) => {
			const diff = diffService.getDiff(projectId, change).response;
			return sum + (diff?.type === "Patch" ? diff.subject.linesRemoved : 0);
		}, 0),
	);

	function selectChange(index: number) {
		selectedIndex = index;
		if (!singleDiffView) {
			// In virtual list mode, scroll to the selected file
			virtualList?.jumpToIndex(index);
		}
	}
</script>

<FloatingModal
	defaults={{
		width: 1000,
		height: 680,
		snapPosition: "center",
		minWidth: 600,
		minHeight: 400,
	}}
	dragHandleElement={headerElRef}
	onCancel={onclose}
>
	<div class="floating-diff-modal">
		<!-- Left panel: drag handle + file list -->
		<div class="left-panel" bind:this={leftPanelEl}>
			{#if leftPanelEl}
				<Resizer
					viewport={leftPanelEl}
					direction="right"
					persistId="floating-diff-modal-left-panel-width"
					minWidth={10}
					maxWidth={30}
					defaultValue={15}
					showBorder
				/>
			{/if}
			<div class="left-header" bind:this={headerElRef}>
				<div class="drag-handle">
					<Icon name="drag-square" />
				</div>
				<ChangedFileStats
					title="Files changed"
					bind:mode={listMode}
					persistId="floating-diff-modal"
					fileCount={changes.length}
					linesAdded={totalLinesAdded}
					linesRemoved={totalLinesRemoved}
				/>
			</div>
			<div class="file-list" bind:this={fileListEl}>
				<ChangedFilesContextMenu
					bind:this={fileListContextMenu}
					{projectId}
					{stackId}
					{selectionId}
					trigger={fileListEl}
				/>
				<FileTreeList
					{changes}
					{listMode}
					{selectedIndex}
					{visibleRange}
					onFileClick={selectChange}
					onFileContextMenu={(e, change) => {
						fileListContextMenu?.open(e, { changes: [change] });
					}}
				/>
			</div>
		</div>

		<!-- Right panel: diff area -->
		<div class="right-panel">
			<div class="floating-actions">
				<Button kind="ghost" icon="cross" size="tag" onclick={onclose} />
			</div>
			<!-- Diff area (single-file or virtual list depending on user setting) -->
			<div class="diff-area" bind:this={diffScrollContainer}>
				{#if singleDiffView}
					<!-- Single-file mode: show selected file with header -->
					{#if selectedChange}
						{@const diffQuery = diffService.getDiff(projectId, selectedChange)}
						<Drawer noshrink stickyHeader scrollRoot={diffScrollContainer} highlighted={false}>
							{#snippet header()}
								<div class="diff-preview-header">
									<FileViewHeader
										filePath={selectedChange.path}
										fileStatus={computeChangeStatus(selectedChange)}
										executable={isExecutableStatus(selectedChange.status)}
									/>
									<div class="diff-preview-header-actions">
										<ChangedFilesContextMenu
											bind:this={contextMenus[selectedChange.path]}
											{projectId}
											{stackId}
											{selectionId}
											leftClickTrigger={buttonElements[selectedChange.path]}
											trigger={buttonElements[selectedChange.path]}
											onopen={() => {
												menuOpenStates[selectedChange.path] = true;
											}}
											onclose={() => {
												menuOpenStates[selectedChange.path] = false;
											}}
										/>
										<Button
											bind:el={buttonElements[selectedChange.path]}
											kind="ghost"
											icon="kebab"
											size="tag"
											activated={menuOpenStates[selectedChange.path]}
											onclick={async () => {
												const contextMenu = contextMenus[selectedChange.path];
												const buttonEl = buttonElements[selectedChange.path];
												if (!contextMenu || !buttonEl) return;

												const allChanges = await idSelection.treeChanges(projectId, selectionId);
												if (
													idSelection.has(selectedChange.path, selectionId) &&
													allChanges.length > 0
												) {
													contextMenu.open(buttonEl, { changes: allChanges });
												} else {
													contextMenu.open(buttonEl, { changes: [selectedChange] });
												}
											}}
										/>
									</div>
								</div>
							{/snippet}

							{#snippet actions()}
								<!-- actions slot intentionally empty; kebab is in header -->
							{/snippet}

							<ReduxResult {projectId} hideLoading result={diffQuery.result}>
								{#snippet children(diff)}
									<UnifiedDiffView
										{projectId}
										{stackId}
										commitId={selectionId.type === "commit" ? selectionId.commitId : undefined}
										{draggable}
										change={selectedChange}
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
					{:else}
						<div class="no-file-selected">
							<p class="text-13 text-body">Select a file to preview its diff</p>
						</div>
					{/if}
				{:else}
					<!-- All-in-one mode: virtual list of all diffs, file list navigates by scrolling -->
					<VirtualList
						bind:this={virtualList}
						startIndex={initialIndex}
						grow
						items={changes}
						defaultHeight={102}
						visibility="scroll"
						renderDistance={100}
						getId={(change) => change.path}
						onVisibleChange={(range) => {
							visibleRange = range;
							if (range) selectedIndex = range.start;
						}}
					>
						{#snippet template(change, index)}
							{@const diffQuery = diffService.getDiff(projectId, change)}
							{@const diffData = diffQuery.response}
							{@const isExecutable = isExecutableStatus(change.status)}
							{@const patchData = diffData?.type === "Patch" ? diffData.subject : null}
							{@const isCollapsed = diffExpandedState.get(change.path) ?? false}
							<Drawer
								noshrink
								stickyHeader
								reserveSpaceOnStuck
								closeButtonPlaceholder
								closeButtonPlaceholderWidth="1.375rem"
								scrollRoot={diffScrollContainer}
								collapsable
								defaultCollapsed={isCollapsed}
								highlighted={selectedIndex === index}
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

											const allChanges = await idSelection.treeChanges(projectId, selectionId);
											if (idSelection.has(change.path, selectionId) && allChanges.length > 0) {
												contextMenu.open(buttonEl, { changes: allChanges });
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
					</VirtualList>
				{/if}
			</div>
		</div>
	</div>
</FloatingModal>

<style>
	.floating-diff-modal {
		display: flex;
		flex-direction: row;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	/* Left panel */
	.left-panel {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		width: 15rem;
		overflow: hidden;
	}

	.left-header {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		height: 42px;
		padding: 0 8px 0 12px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background: var(--clr-bg-2);
		cursor: grab;
	}

	.drag-handle {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
		color: var(--clr-text-3);
	}

	/* File list */
	.file-list {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow-y: auto;
		background-color: var(--clr-bg-1);
	}

	/* Right panel */
	.right-panel {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
	}

	.floating-actions {
		display: flex;
		z-index: var(--z-lifted);
		position: absolute;
		top: 6px;
		right: 6px;
		padding: 2px;
		gap: 2px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);
		box-shadow: var(--fx-shadow-s);
	}

	/* Diff area (shared scroll root for virtual list, or single-file container) */
	.diff-area {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow-x: hidden;
		overflow-y: auto;
	}

	/* Single-file mode header */
	.diff-preview-header {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: space-between;
		overflow: hidden;
		gap: 8px;
	}

	.diff-preview-header-actions {
		display: flex;
		flex-shrink: 0;
		align-items: center;
	}

	.no-file-selected {
		display: flex;
		flex: 1;
		align-items: center;
		justify-content: center;
		color: var(--clr-text-3);
	}

	/* Virtual list mode: ensure FileViewHeader fills drawer header */
	.full-width {
		width: 100%;
	}

	.loading {
		height: 130px;
		padding: 12px;
	}
</style>
