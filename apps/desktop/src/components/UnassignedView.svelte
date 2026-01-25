<script lang="ts">
	import RulesList from '$components/RulesList.svelte';
	import UnassignedFoldButton from '$components/UnassignedFoldButton.svelte';
	import UnassignedViewForgePrompt from '$components/UnassignedViewForgePrompt.svelte';
	import WorktreeChanges from '$components/WorktreeChanges.svelte';
	import { ActionEvent, POSTHOG_WRAPPER } from '$lib/analytics/posthog';
	import noChanges from '$lib/assets/empty-state/no-new-changes.svg?raw';
	import { stagingBehaviorFeature } from '$lib/config/uiFeatureFlags';
	import { FILE_SELECTION_MANAGER } from '$lib/selection/fileSelectionManager.svelte';
	import { createWorktreeSelection } from '$lib/selection/key';
	import { UNCOMMITTED_SERVICE } from '$lib/selection/uncommittedService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Badge, Button, TestId } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';

	interface Props {
		projectId: string;
		onFileClick?: (index: number) => void;
	}

	const { projectId, onFileClick }: Props = $props();

	const selectionId = createWorktreeSelection({ stackId: undefined });

	const uiState = inject(UI_STATE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);
	const posthog = inject(POSTHOG_WRAPPER);
	const projectState = $derived(uiState.project(projectId));
	const unassignedSidebarFolded = $derived(uiState.global.unassignedSidebarFolded);
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(exclusiveAction?.type === 'commit');

	const treeChanges = $derived(uncommittedService.changesByStackId(null));
	const treeChangesCount = $derived(treeChanges.current.length);
	const changesToCommit = $derived(treeChangesCount > 0);
	let foldedContentWidth = $state<number>(0);

	function unfoldView() {
		unassignedSidebarFolded.set(false);
	}

	function unselectFiles() {
		idSelection.clear(selectionId);
	}

	$effect(() => {
		if (isCommitting && changesToCommit) {
			unassignedSidebarFolded.set(false);
		}
	});

	function foldUnnassignedView() {
		unassignedSidebarFolded.set(true);
	}

	function checkSelectedFilesForCommit() {
		const selectionId = createWorktreeSelection({});
		const selectedPaths = idSelection.values(selectionId).map((entry) => entry.path);

		// If there are selected paths in the unassigned selection, we check those.
		if (selectedPaths.length > 0) {
			uncommittedService.checkFiles(null, selectedPaths);
		} else {
			uncommittedService.checkAll(null);
		}
	}

	function uncheckAll() {
		uncommittedService.uncheckAll(null);
	}

	function checkAllFiles() {
		uncommittedService.checkAll(null);
	}

	function checkFilesForCommit(): true {
		switch ($stagingBehaviorFeature) {
			case 'all':
				checkAllFiles();
				return true;
			case 'selection':
				// We only check the selected files.
				checkSelectedFilesForCommit();
				return true;
			case 'none':
				uncheckAll();
				return true;
		}
	}
</script>

{#snippet foldButton()}
	{#if !isCommitting && !unassignedSidebarFolded.current}
		<div class="unassigned-fold-button">
			<UnassignedFoldButton active={false} onclick={foldUnnassignedView} />
		</div>
	{/if}
{/snippet}

{#if !unassignedSidebarFolded.current}
	<div class="unassigned" role="presentation" use:focusable={{ vertical: true }}>
		<div class="unassigned-wrap">
			<div role="presentation" class="unassigned-files-wrapper" onclick={unselectFiles}>
				<WorktreeChanges
					title="Unstaged"
					{projectId}
					stackId={undefined}
					mode="unassigned"
					{foldButton}
					{onFileClick}
				>
					{#snippet emptyPlaceholder()}
						<div class="unassigned-empty">
							{@html noChanges}
							<p class="text-13 text-body unassigned-empty-text">
								You're all caught up!<br />
								No files need committing
							</p>
						</div>
					{/snippet}
				</WorktreeChanges>
			</div>

			<UnassignedViewForgePrompt {projectId} />

			{#if changesToCommit}
				<div class="create-new" use:focusable>
					<Button
						type="button"
						wide
						reversedDirection
						disabled={!!projectState.exclusiveAction.current}
						onclick={() => {
							projectState.exclusiveAction.set({
								type: 'commit',
								stackId: undefined,
								branchName: undefined
							});
							checkFilesForCommit();
							posthog.captureAction(ActionEvent.CommitToNewBranch);
						}}
						icon={isCommitting ? undefined : 'plus-small'}
						testId={TestId.CommitToNewBranchButton}
						kind="outline"
					>
						{#if isCommitting}
							Committing…
						{:else}
							Commit to new branch
						{/if}
					</Button>
				</div>
			{/if}
		</div>

		<RulesList {projectId} />
	</div>
{:else}
	<div
		role="presentation"
		class="unassigned-folded"
		ondblclick={unfoldView}
		class:changes-to-commit={changesToCommit}
		use:focusable={{ vertical: true }}
	>
		<UnassignedFoldButton active={true} onclick={unfoldView} />

		<div class="unassigned-folded-content">
			<Badge>
				{treeChangesCount > 99 ? '99+' : treeChangesCount}
			</Badge>
			<span
				bind:clientWidth={foldedContentWidth}
				style="height: {foldedContentWidth}px;"
				class="unassigned-folded-text text-14 text-semibold">Unstaged</span
			>
		</div>
	</div>
{/if}

<style lang="postcss">
	.unassigned-empty {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 0 20px 40px;
		gap: 20px;
	}

	.unassigned-empty-text {
		color: var(--clr-text-3);
		text-align: center;
	}

	.unassigned {
		display: flex;
		flex-direction: column;
		height: calc(100% + 1px);
		margin-bottom: -1px;
		overflow: hidden;
		background-color: var(--clr-bg-1);
	}

	.unassigned-wrap {
		display: flex;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.create-new {
		display: flex;
		flex-direction: column;
		padding: 12px 12px 14px 12px;
		border-top: 1px solid var(--clr-border-3);
	}

	/* FOLDED */
	.unassigned-folded {
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		padding: 11px 0;
		gap: 10px;

		&.changes-to-commit {
			background-color: var(--clr-bg-1);
		}
	}

	.unassigned-folded-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		height: 100%;
		gap: 10px;
	}

	.unassigned-folded-text {
		display: flex;
		align-items: center;
		writing-mode: vertical-lr;
	}

	.unassigned-files-wrapper {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		overflow: hidden;
	}

	/* MODIFIERS */

	.unassigned-fold-button {
		display: flex;
		/* Align this icon's position with the folded one.
		Prevent any position shifting or jumping. */
		margin-left: -3px;
	}
</style>
