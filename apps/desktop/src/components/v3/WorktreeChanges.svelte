<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import FileListMode from '$components/v3/FileListMode.svelte';
	import WorktreeTipsFooter from '$components/v3/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { createCommitStore } from '$lib/commits/contexts';
	import { Focusable, FocusManager } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { ChangeSelectionService, type SelectedFile } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';

	type Props = {
		projectId: string;
		stackId?: string;
	};

	let { projectId, stackId }: Props = $props();

	const [changeSelection, worktreeService, uiState, stackService, focusManager] = inject(
		ChangeSelectionService,
		WorktreeService,
		UiState,
		StackService,
		FocusManager
	);

	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage.get());
	const isCommitting = $derived(drawerPage.current === 'new-commit');
	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);
	const defaultBranchResult = $derived(
		stackId !== undefined ? stackService.defaultBranch(projectId, stackId) : undefined
	);
	const defaultBranch = $derived(defaultBranchResult?.current.data);
	const defaultBranchName = $derived(defaultBranch?.name);
	const selectedChanges = changeSelection.list();
	const noChangesSelected = $derived(selectedChanges.current.length === 0);
	const changesResult = $derived(worktreeService.getChanges(projectId));
	const affectedPaths = $derived(changesResult.current.data?.map((c) => c.path));

	let focusGroup = focusManager.radioGroup({
		triggers: [Focusable.UncommittedChanges, Focusable.ChangedFiles]
	});
	const listActive = $derived(focusGroup.current === Focusable.ChangedFiles);

	const filesFullySelected = $derived(
		changeSelection.every(affectedPaths ?? [], (f) => f.type === 'full')
	);

	const filesPartiallySelected = $derived(!noChangesSelected && !filesFullySelected);

	// TODO: Make this go away.
	createCommitStore(undefined);

	/** Clear any selected changes that no longer exist. */
	$effect(() => {
		changeSelection.retain(affectedPaths);
	});

	let listMode: 'list' | 'tree' = $state('list');

	function selectEverything() {
		const affectedPaths =
			changesResult.current.data?.map((c) => [c.path, c.pathBytes] as const) ?? [];
		const files: SelectedFile[] = affectedPaths.map(([path, pathBytes]) => ({
			path,
			pathBytes,
			type: 'full'
		}));
		changeSelection.addMany(files);
	}

	function updateCommitSelection() {
		if (!noChangesSelected) return;
		// If no changes are selected, select everything.
		selectEverything();
	}

	function startCommit() {
		updateCommitSelection();
		projectState.drawerPage.set('new-commit');
		if (defaultBranchName) {
			stackState?.selection.set({ branchName: defaultBranchName });
		}
	}

	function toggleGlobalCheckbox() {
		if (noChangesSelected) {
			selectEverything();
			return;
		}
		changeSelection.clear();
	}

	let isFooterSticky = $state(false);
</script>

<ReduxResult {stackId} {projectId} result={changesResult.current}>
	{#snippet children(changes, { stackId, projectId })}
		<ScrollableContainer wide>
			<div
				class="uncommitted-changes-wrap"
				use:focusable={{ id: Focusable.UncommittedChanges, parentId: Focusable.WorkspaceLeft }}
			>
				<div use:stickyHeader class="worktree-header">
					<div class="worktree-header__general">
						{#if isCommitting}
							<Checkbox
								checked={filesPartiallySelected || filesFullySelected}
								indeterminate={filesPartiallySelected}
								small
								onchange={toggleGlobalCheckbox}
							/>
						{/if}
						<div class="worktree-header__title truncate">
							<h3 class="text-14 text-semibold truncate">Uncommitted</h3>
							{#if changes.length > 0}
								<Badge>{changes.length}</Badge>
							{/if}
						</div>
					</div>
					<FileListMode bind:mode={listMode} persist="uncommitted" />
				</div>
				{#if changes.length > 0}
					<div class="uncommitted-changes">
						<FileList
							selectionId={{ type: 'worktree' }}
							showCheckboxes={isCommitting}
							{projectId}
							{stackId}
							{changes}
							{listMode}
							{listActive}
						/>
					</div>
					<div
						use:stickyHeader={{ align: 'bottom' }}
						class="start-commit"
						class:sticked={isFooterSticky}
					>
						<Button
							kind={isCommitting ? 'outline' : 'solid'}
							type="button"
							size="cta"
							wide
							disabled={isCommitting}
							onclick={startCommit}
						>
							Start a commit…
						</Button>
					</div>
				{:else}
					<div class="uncommitted-changes__empty">
						<div class="uncommitted-changes__empty__placeholder">
							{@html noChanges}
							<p class="text-13 text-body uncommitted-changes__empty__placeholder-text">
								You're all caught up!<br />
								No files need committing
							</p>
						</div>

						<WorktreeTipsFooter />
					</div>
				{/if}
			</div>
		</ScrollableContainer>
	{/snippet}
</ReduxResult>

<style>
	.uncommitted-changes-wrap {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.worktree-header {
		display: flex;
		padding: 10px 10px 10px 14px;
		width: 100%;
		align-items: center;
		text-wrap: nowrap;
		justify-content: space-between;
		white-space: nowrap;
		gap: 8px;
		background-color: var(--clr-bg-1);
	}

	.worktree-header__general {
		display: flex;
		align-items: center;
		gap: 10px;
		overflow: hidden;
	}

	.worktree-header__title {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.uncommitted-changes {
		display: flex;
		flex-direction: column;
		flex: 1;
		width: 100%;
	}

	.start-commit {
		position: sticky;
		bottom: -1px;
		padding: 16px;
		background-color: var(--clr-bg-1);

		&.sticked {
			border-top: 1px solid var(--clr-border-2);
		}
	}

	.uncommitted-changes__empty {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.uncommitted-changes__empty__placeholder {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 20px;
		padding: 0 20px 40px;
	}

	.uncommitted-changes__empty__placeholder-text {
		text-align: center;
		color: var(--clr-text-3);
	}
</style>
