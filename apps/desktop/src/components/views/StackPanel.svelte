<!--
	Compound child that renders the left panel of a stack view.
	Contains: worktree changes, start commit button, IRC row, and branch list.
	Reads shared state from StackController via context.

	Usage:
	```svelte
	<StackPanel {branches} {topBranchName} {onFoldStack} {ircEnabled} {ircChannel} />
	```
-->
<script lang="ts">
	import NewCommitView from "$components/commit/NewCommitView.svelte";
	import WorktreeChanges from "$components/files/WorktreeChanges.svelte";
	import IrcRow from "$components/irc/IrcRow.svelte";
	import StackDragHandle from "$components/stack/StackDragHandle.svelte";
	import BranchList from "$components/views/BranchList.svelte";
	import { stagingBehaviorFeature } from "$lib/config/uiFeatureFlags";
	import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
	import { createWorktreeSelection, type SelectionId } from "$lib/selection/key";
	import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
	import { getStackContext } from "$lib/stacks/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, TestId } from "@gitbutler/ui";
	import type { BranchDetails } from "$lib/stacks/stack";

	type Props = {
		branches: BranchDetails[];
		topBranchName?: string;
		onFoldStack?: () => void;
		ircEnabled: boolean;
		ircChannel?: string;
	};

	const { branches, topBranchName, onFoldStack, ircEnabled, ircChannel }: Props = $props();

	const controller = getStackContext();
	const stackService = inject(STACK_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	const changes = $derived(uncommittedService.changesByStackId(controller.stackId || null));
	const startCommitVisible = $derived(uncommittedService.startCommitVisible(controller.stackId));
	const defaultBranchQuery = $derived(
		stackService.defaultBranch(controller.projectId, controller.stackId),
	);
	const defaultBranch = $derived(defaultBranchQuery?.response);

	let dropzoneActivated = $state(false);
	let dropzoneHovered = $state(false);

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

		const selectionId = createWorktreeSelection({});
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

	function startCommit(branchName: string) {
		controller.projectState.exclusiveAction.set({
			type: "commit",
			branchName,
			stackId: controller.stackId,
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
	/>

	<div
		class="assignments-wrap"
		class:assignments__empty={changes.current.length === 0 && !controller.isCommitting}
	>
		<div
			class="worktree-wrap"
			class:remove-border-bottom={(controller.isCommitting && changes.current.length === 0) ||
				!startCommitVisible.current}
			class:dropzone-activated={dropzoneActivated && changes.current.length === 0}
			class:dropzone-hovered={dropzoneHovered && changes.current.length === 0}
		>
			<WorktreeChanges
				title="Staged"
				projectId={controller.projectId}
				stackId={controller.stackId}
				mode="assigned"
				visibleRange={controller.visibleRange}
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
								Drop files to stage or commit directly
							</p>
						</div>
					{/if}
				{/snippet}
			</WorktreeChanges>
		</div>

		{#if startCommitVisible.current || controller.isCommitting}
			{#if !controller.isCommitting}
				<div class="start-commit">
					<Button
						testId={TestId.StartCommitButton}
						kind={changes.current.length > 0 ? "solid" : "outline"}
						style={changes.current.length > 0 ? "pop" : "gray"}
						type="button"
						wide
						disabled={controller.isReadOnly ||
							defaultBranch === null ||
							!!controller.exclusiveAction}
						tooltip={controller.isReadOnly ? "Read-only mode" : undefined}
						onclick={() => {
							if (defaultBranch) startCommit(defaultBranch);
						}}
					>
						Start a commit…
					</Button>
				</div>
			{:else if controller.isCommitting}
				<NewCommitView projectId={controller.projectId} stackId={controller.stackId} />
			{/if}
		{/if}
	</div>

	{#if ircEnabled && topBranchName}
		<IrcRow stackId={controller.stackId} channel={ircChannel} selected={controller.ircPanelOpen} />
	{/if}

	<BranchList {branches} />
</div>

<style lang="postcss">
	.stack-view__inner {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.assignments-wrap {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.worktree-wrap {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml) var(--radius-ml) 0 0;

		&.remove-border-bottom {
			border-bottom: none;
		}

		&.dropzone-activated {
			& .assigned-changes-empty {
				padding: 20px 8px 20px;
				background-color: var(--clr-bg-1);
				transition:
					background-color var(--transition-fast),
					padding var(--transition-fast);
			}

			& .assigned-changes-empty__text {
				color: var(--clr-theme-pop-on-soft);
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
		background-color: var(--clr-bg-1);
	}

	/* EMPTY ASSIGN AREA */
	:global(.assigned-changes-empty) {
		display: flex;
		position: relative;
		padding: 10px 8px;
		overflow: hidden;
		gap: 12px;
		background-color: var(--clr-bg-2);
		transition: background-color var(--transition-fast);
	}

	:global(.assigned-changes-empty__text) {
		width: 100%;
		color: var(--clr-text-2);
		text-align: center;
		opacity: 0.7;
		transition:
			color var(--transition-fast),
			opacity var(--transition-fast);
	}
</style>
