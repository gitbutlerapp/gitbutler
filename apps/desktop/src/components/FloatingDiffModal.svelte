<script lang="ts">
	import ChangedFileStats from "$components/ChangedFileStats.svelte";
	import ChangedFilesContextMenu from "$components/ChangedFilesContextMenu.svelte";
	import Drawer from "$components/Drawer.svelte";
	import FileTreeList from "$components/FileTreeList.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import Resizer from "$components/Resizer.svelte";
	import UnifiedDiffView from "$components/UnifiedDiffView.svelte";
	import { FileContextMenuState } from "$lib/diffState/fileContextMenuState.svelte";
	import FloatingModal from "$lib/floating/FloatingModal.svelte";
	import { isExecutableStatus, type TreeChange } from "$lib/hunks/change";
	import { DIFF_SERVICE } from "$lib/hunks/diffService.svelte";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { type SelectionId } from "$lib/selection/key";
	import { SETTINGS } from "$lib/settings/userSettings";
	import { computeChangeStatus } from "$lib/utils/fileStatus";
	import { inject } from "@gitbutler/core/context";
	import { Button, FileViewHeader, HunkDiffSkeleton, Icon, VirtualList } from "@gitbutler/ui";
	import { FOCUS_MANAGER, type FocusableOptions } from "@gitbutler/ui/focus/focusManager";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { tick } from "svelte";

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
	const focusManager = inject(FOCUS_MANAGER);

	const allInOneDiff = $derived($userSettings.allInOneDiff);
	const highlightDiffs = $derived($userSettings.highlightDiffs);

	// Not reactive by design — feeds `defaultCollapsed` as an initial value only,
	// so Svelte does not need to track mutations.
	const diffExpandedState = new Map<string, boolean>();

	let listMode: "list" | "tree" = $state("list");
	// Seeded from prop once on mount; subsequent user selection is independent.
	let selectedIndex = $state(initialIndex);
	let headerElRef = $state<HTMLDivElement | undefined>(undefined);
	let leftPanelEl = $state<HTMLDivElement | undefined>(undefined);
	let virtualList = $state<VirtualList<TreeChange>>();
	let diffScrollContainer = $state<HTMLElement | null>(null);
	let fileListEl = $state<HTMLDivElement>();
	let fileListContextMenu = $state<ReturnType<typeof ChangedFilesContextMenu>>();
	let visibleRange = $state<{ start: number; end: number } | undefined>(undefined);
	const menuState = new FileContextMenuState<ReturnType<typeof ChangedFilesContextMenu>>();

	const selectedChange = $derived(changes[selectedIndex] ?? changes[0]);

	function selectChange(index: number) {
		selectedIndex = index;
		if (allInOneDiff) {
			// In virtual list mode, scroll to the selected file
			virtualList?.jumpToIndex(index);
		}
	}

	function getItemFocusableOpts(index: number): FocusableOptions {
		return {
			focusable: true,
			onActive: (value) => {
				if (value) selectChange(index);
			},
			onEsc: () => {
				onclose();
				return true;
			},
		};
	}

	// After all components (including FloatingModal's trap container) have mounted,
	// explicitly re-register the initial item with activate:true so it claims
	// currentNode from the modal's own activate:true trap.
	$effect(() => {
		if (!fileListEl) return;
		tick().then(() => {
			const items = fileListEl?.querySelectorAll<HTMLElement>(".file-list-item");
			const targetEl = items?.[initialIndex];
			if (targetEl) {
				focusManager.register({ ...getItemFocusableOpts(initialIndex), activate: true }, targetEl);
			}
		});
	});
</script>

<FloatingModal
	persistId="floating-diff-modal-size"
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
					minWidth={14}
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
				/>
			</div>
			<div class="file-list" bind:this={fileListEl} use:focusable={{ vertical: true }}>
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
					{getItemFocusableOpts}
					onFileClick={selectChange}
					onFileContextMenu={(e, change) => {
						fileListContextMenu?.open(e, { changes: [change] });
					}}
				/>
			</div>
		</div>

		<!-- Right panel: diff area -->
		<div class="right-panel">
			{#if allInOneDiff}
				<div class="floating-actions">
					<Button kind="ghost" icon="cross" size="tag" onclick={onclose} />
				</div>
			{/if}
			<!-- Diff area (single-file or virtual list depending on user setting) -->
			<div class="diff-area" bind:this={diffScrollContainer}>
				{#if !allInOneDiff}
					<!-- Single-file mode: show selected file with header -->
					{#if selectedChange}
						{@const diffQuery = diffService.getDiff(projectId, selectedChange)}
						<Drawer
							noshrink
							stickyHeader
							collapsable={false}
							scrollRoot={diffScrollContainer}
							highlighted={false}
							{onclose}
						>
							{#snippet header()}
								<FileViewHeader
									filePath={selectedChange.path}
									fileStatus={computeChangeStatus(selectedChange)}
									executable={isExecutableStatus(selectedChange.status)}
								/>
							{/snippet}

							{#snippet actions()}
								<ChangedFilesContextMenu
									bind:this={menuState.contextMenus[selectedChange.path]}
									{projectId}
									{stackId}
									{selectionId}
									leftClickTrigger={menuState.buttonElements[selectedChange.path]}
									trigger={menuState.buttonElements[selectedChange.path]}
									onopen={() => {
										menuState.menuOpenStates[selectedChange.path] = true;
									}}
									onclose={() => {
										menuState.menuOpenStates[selectedChange.path] = false;
									}}
								/>
								<Button
									bind:el={menuState.buttonElements[selectedChange.path]}
									kind="ghost"
									icon="kebab"
									size="tag"
									activated={menuState.menuOpenStates[selectedChange.path]}
									onclick={async () => {
										const contextMenu = menuState.contextMenus[selectedChange.path];
										const buttonEl = menuState.buttonElements[selectedChange.path];
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
								highlighted={allInOneDiff && highlightDiffs && selectedIndex === index}
								ontoggle={(collapsed) => {
									diffExpandedState.set(change.path, collapsed);
								}}
							>
								{#snippet header()}
									<div class="full-width" bind:this={menuState.headerTriggers[change.path]}>
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
										bind:this={menuState.contextMenus[change.path]}
										{projectId}
										{stackId}
										{selectionId}
										leftClickTrigger={menuState.buttonElements[change.path]}
										trigger={menuState.headerTriggers[change.path]}
										onopen={() => {
											menuState.menuOpenStates[change.path] = true;
										}}
										onclose={() => {
											menuState.menuOpenStates[change.path] = false;
										}}
									/>
									<Button
										bind:el={menuState.buttonElements[change.path]}
										kind="ghost"
										icon="kebab"
										size="tag"
										activated={menuState.menuOpenStates[change.path]}
										onclick={async () => {
											const contextMenu = menuState.contextMenus[change.path];
											const buttonEl = menuState.buttonElements[change.path];
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

	/* Virtual list mode: ensure FileViewHeader fills drawer header */
	.full-width {
		width: 100%;
	}

	.loading {
		height: 130px;
		padding: 12px;
	}
</style>
