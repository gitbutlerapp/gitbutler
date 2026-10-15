<!--
	Compound child that renders the left panel of a stack view.
	Contains: worktree changes, start commit button, and branch list. In a
	linked worktree's lane these are that worktree's files and commit box.
	Reads shared state from StackController via context.

	Usage:
	```svelte
	<StackPanel {segments} {topBranchName} {onFoldStack} />
	```
-->
<script lang="ts">
	import NewCommitView from "$components/commit/NewCommitView.svelte";
	import WorktreeChanges from "$components/files/WorktreeChanges.svelte";
	import Resizer from "$components/shared/Resizer.svelte";
	import SashLayer from "$components/shared/SashLayer.svelte";
	import RemoveWorktreeModal from "$components/stack/RemoveWorktreeModal.svelte";
	import StackDragHandle from "$components/stack/StackDragHandle.svelte";
	import BranchList from "$components/views/BranchList.svelte";
	import { CLIPBOARD_SERVICE } from "$lib/backend/clipboard";
	import { stagingBehaviorFeature } from "$lib/config/uiFeatureFlags";
	import { FILE_SERVICE } from "$lib/files/fileService";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { createWorktreeSelection, type SelectionId } from "$lib/selection/key";
	import { getStackContext } from "$lib/stacks/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, ContextMenuItem, ContextMenuSection, TestId } from "@gitbutler/ui-svelte";
	import type { Segment } from "@gitbutler/but-sdk";

	type Props = {
		segments: Segment[];
		topBranchName?: string;
		onFoldStack?: () => void;
		/** In a worktree's lane, the stack branch the worktree rests on. */
		baseBranchName?: string;
	};

	const { segments, topBranchName, onFoldStack, baseBranchName }: Props = $props();

	const controller = getStackContext();
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const stackService = inject(STACK_SERVICE);
	const fileService = inject(FILE_SERVICE);
	const clipboardService = inject(CLIPBOARD_SERVICE);

	let removeWorktreeModal = $state<RemoveWorktreeModal>();

	const worktree = $derived(controller.worktree);
	const topBranchNote = $derived.by(() => {
		if (!worktree) return undefined;
		if (!baseBranchName) return "worktree";
		return `worktree on ${baseBranchName}`;
	});
	const uncommittedService = $derived(controller.uncommitted);
	const changes = $derived(uncommittedService.changesByStackId(controller.stackId || null));
	const startCommitVisible = $derived(uncommittedService.startCommitVisible(controller.stackId));
	// A worktree without a branch commits onto its detached `HEAD`.
	const canStartCommit = $derived(
		!controller.isReadOnly && !controller.exclusiveAction && (!!topBranchName || !!worktree),
	);

	let dropzoneActivated = $state(false);
	let dropzoneHovered = $state(false);
	let stagedListEl = $state<HTMLDivElement | undefined>();
	let stagedHeight = $state("40vh");
	let stagedContentHeightRem = $state(60);

	$effect(() => {
		const viewport = stagedListEl?.querySelector<HTMLElement>(".viewport");
		if (!viewport) return;
		const observer = new ResizeObserver(() => {
			stagedContentHeightRem = viewport.scrollHeight / 16;
		});
		observer.observe(viewport);
		return () => observer.disconnect();
	});

	function selectedPathsForSelection(selectionId: SelectionId): string[] {
		return idSelection.values(selectionId).map((entry) => entry.path);
	}

	function checkSelectedFilesForCommit() {
		const stackAssignments = controller.stackId
			? uncommittedService.getAssignmentsByStackId(controller.stackId)
			: [];
		if (controller.stackId && stackAssignments.length > 0) {
			const selectionId = createWorktreeSelection({ stackId: controller.stackId });
			const selectedPaths = selectedPathsForSelection(selectionId);

			if (selectedPaths.length > 0) {
				uncommittedService.checkFiles(controller.stackId, selectedPaths);
			} else {
				uncommittedService.checkAll(controller.stackId);
			}
			uncommittedService.uncheckAll(null);
			return;
		}

		const selectionId = createWorktreeSelection({ worktree: worktree?.name });
		const selectedPaths = selectedPathsForSelection(selectionId);

		if (selectedPaths.length > 0) {
			uncommittedService.checkFiles(null, selectedPaths);
		} else {
			uncommittedService.checkAll(null);
		}
	}

	function uncheckAll() {
		if (controller.stackId) {
			uncommittedService.uncheckAll(controller.stackId);
		}
		uncommittedService.uncheckAll(null);
	}

	function checkAllFiles() {
		const stackAssignments = controller.stackId
			? uncommittedService.getAssignmentsByStackId(controller.stackId)
			: [];
		if (controller.stackId && stackAssignments.length > 0) {
			uncommittedService.checkAll(controller.stackId);
			uncommittedService.uncheckAll(null);
			return;
		}

		uncommittedService.checkAll(null);
	}

	function checkFilesForCommit(): true {
		switch ($stagingBehaviorFeature) {
			case "all":
				checkAllFiles();
				return true;
			case "selection":
				checkSelectedFilesForCommit();
				return true;
			case "none":
				uncheckAll();
				return true;
		}
	}

	function startCommit() {
		controller.projectState.exclusiveAction.set({
			type: "commit",
			branchName: topBranchName,
			stackId: controller.stackId,
			worktree: worktree?.name,
		});

		checkFilesForCommit();
	}
</script>

<div class="stack-v stack-view__inner">
	<StackDragHandle
		stackId={controller.stackId}
		projectId={controller.projectId}
		disabled={controller.isCommitting}
		onFold={onFoldStack}
		branchName={topBranchName}
		draggable={!worktree}
		menu={worktree ? worktreeMenu : undefined}
	/>

	{#snippet worktreeMenu({ close }: { close: () => void })}
		{#if worktree}
			<ContextMenuSection>
				<ContextMenuItem
					label={fileService.showInFolderLabel}
					icon="open-in-folder"
					onclick={async () => {
						close();
						await fileService.showFileInFolder(controller.projectId, worktree.name, "");
					}}
				/>
				<ContextMenuItem
					label="Copy path"
					icon="copy"
					onclick={async () => {
						close();
						const path = await fileService.workdirPath(controller.projectId, worktree.name, "");
						await clipboardService.write(path, { message: "Worktree path copied" });
					}}
				/>
			</ContextMenuSection>
			<ContextMenuSection>
				<ContextMenuItem
					label="Archive worktree"
					icon="eject"
					onclick={async () => {
						close();
						await stackService.setWorktreeArchived({
							projectId: controller.projectId,
							name: worktree.name,
							archived: true,
						});
					}}
				/>
				<ContextMenuItem
					label="Remove worktree…"
					icon="bin"
					onclick={() => {
						close();
						removeWorktreeModal?.show();
					}}
				/>
			</ContextMenuSection>
		{/if}
	{/snippet}

	{#if worktree}
		<RemoveWorktreeModal
			bind:this={removeWorktreeModal}
			projectId={controller.projectId}
			name={worktree.name}
		/>
	{/if}

	<div
		class="assignments-wrap"
		class:assignments__empty={changes.current.length === 0 && !controller.isCommitting}
	>
		<SashLayer>
			<div
				class="worktree-wrap"
				class:remove-border-bottom={(controller.isCommitting && changes.current.length === 0) ||
					!startCommitVisible.current}
				class:dropzone-activated={dropzoneActivated && changes.current.length === 0}
				class:dropzone-hovered={dropzoneHovered && changes.current.length === 0}
			>
				<WorktreeChanges
					title={worktree ? "Uncommitted" : "Staged"}
					projectId={controller.projectId}
					stackId={controller.stackId}
					worktree={worktree?.name}
					mode="assigned"
					visibleRange={controller.visibleRange}
					maxListHeight={stagedHeight}
					bind:scrollAreaEl={stagedListEl}
					onDropzoneActivated={(activated) => {
						dropzoneActivated = activated;
					}}
					onDropzoneHovered={(hovered) => {
						dropzoneHovered = hovered;
					}}
					onFileClick={(index) => {
						controller.selection.set(undefined);
						controller.jumpToIndex(index);
					}}
				>
					{#snippet emptyPlaceholder()}
						{#if !controller.isCommitting}
							<div class="assigned-changes-empty">
								<p class="text-12 text-body assigned-changes-empty__text">
									{worktree ? "No uncommitted changes" : "Drop files to stage or commit directly"}
								</p>
							</div>
						{/if}
					{/snippet}
				</WorktreeChanges>
				{#if stagedListEl && changes.current.length > 0}
					<Resizer
						viewport={stagedListEl}
						direction="down"
						defaultValue={undefined}
						minHeight={3}
						maxHeight={stagedContentHeightRem}
						unsetMaxHeight="40vh"
						persistId="staged-changes-height-{controller.laneId}"
						onWidth={(h) => (stagedHeight = `${h}rem`)}
						onDblClick={() => (stagedHeight = "40vh")}
					/>
				{/if}
			</div>
		</SashLayer>

		{#if startCommitVisible.current || controller.isCommitting}
			{#if !controller.isCommitting}
				<div class="start-commit">
					<Button
						testId={TestId.StartCommitButton}
						kind={changes.current.length > 0 ? "solid" : "outline"}
						style={changes.current.length > 0 ? "pop" : "gray"}
						type="button"
						wide
						disabled={!canStartCommit}
						tooltip={controller.readOnlyTooltip}
						onclick={startCommit}
					>
						Start a commit…
					</Button>
				</div>
			{:else if controller.isCommitting}
				<NewCommitView
					projectId={controller.projectId}
					stackId={controller.stackId}
					{worktree}
					laneId={controller.laneId}
				/>
			{/if}
		{/if}
	</div>

	<BranchList {segments} {topBranchNote} />
</div>

<style lang="postcss">
	.stack-view__inner {
		display: flex;
		flex-direction: column;
	}

	.assignments-wrap {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		margin-bottom: 12px;
		overflow: hidden;
		border: 1px solid var(--border-2);
		border-radius: var(--radius-ml);
	}

	.worktree-wrap {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--border-2);
		border-radius: var(--radius-ml) var(--radius-ml) 0 0;

		&.remove-border-bottom {
			border-bottom: none;
		}

		&.dropzone-activated {
			& .assigned-changes-empty {
				background-color: var(--bg-1);
				transition: background-color var(--transition-fast);
			}

			& .assigned-changes-empty__text {
				color: var(--chip-pop-fg);
			}
		}

		&.dropzone-hovered {
			& .assigned-changes-empty__text {
				opacity: 1;
			}
		}
	}

	.start-commit {
		padding: 12px;
		background-color: var(--bg-1);
	}

	/* EMPTY ASSIGN AREA */
	:global(.assigned-changes-empty) {
		display: flex;
		position: relative;
		padding: 10px 8px;
		overflow: hidden;
		gap: 12px;
		background-color: var(--bg-2);
		transition: background-color var(--transition-fast);
	}

	:global(.assigned-changes-empty__text) {
		width: 100%;
		color: var(--text-2);
		text-align: center;
		opacity: 0.7;
		transition:
			color var(--transition-fast),
			opacity var(--transition-fast);
	}
</style>
