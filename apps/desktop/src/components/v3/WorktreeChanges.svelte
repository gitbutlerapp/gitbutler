<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileList from '$components/v3/FileList.svelte';
	import FileListMode from '$components/v3/FileListMode.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { createCommitStore } from '$lib/commits/contexts';
	import { FocusManager } from '$lib/focus/focusManager.svelte';
	import { ChangeSelectionService, type SelectedFile } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Checkbox from '@gitbutler/ui/Checkbox.svelte';

	type Props = {
		projectId: string;
		stackId?: string;
	};

	const { projectId, stackId }: Props = $props();

	const [changeSelection, worktreeService, uiState, stackService, focus] = inject(
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

	const focusedArea = $derived(focus.current);

	let listMode: 'list' | 'tree' = $state('list');

	$effect(() => {
		// If the focused area updates and it matches "left" then we update what
		// selection should be shown in the main view.
		if (focusedArea === 'left') {
			stackState?.activeSelectionId.set({ type: 'worktree' });
		}
	});

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
		if (!defaultBranchName) return;
		stackState?.selection.set({ branchName: defaultBranchName });
		updateCommitSelection();
		projectState.drawerPage.set('new-commit');
	}

	function toggleGlobalCheckbox() {
		if (noChangesSelected) {
			selectEverything();
			return;
		}
		changeSelection.clear();
	}
</script>

<ReduxResult result={changesResult.current}>
	{#snippet children(changes)}
		<div class="worktree-header">
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
					<h3 class="text-14 text-semibold truncate">Uncommitted changes</h3>
					{#if changes.length > 0}
						<Badge>{changes.length}</Badge>
					{/if}
				</div>
			</div>
			<FileListMode bind:mode={listMode} persist="uncommitted" />
		</div>
		{#if changes.length > 0}
			<div class="uncommitted-changes">
				<ScrollableContainer wide>
					<FileList
						selectionId={{ type: 'worktree', showCheckboxes: isCommitting }}
						{projectId}
						{stackId}
						{changes}
						{listMode}
					/>
				</ScrollableContainer>
				<div class="start-commit">
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
			</div>
		{:else}
			<div class="empty-state">
				{@html noChanges}
				<p class="text-13 text-body empty-state-text">
					You're all caught up!<br />
					No files need committing
				</p>
			</div>
		{/if}
	{/snippet}
</ReduxResult>

<style>
	.worktree-header {
		display: flex;
		padding: 10px 10px 10px 14px;
		width: 100%;
		align-items: center;
		text-wrap: nowrap;
		overflow: hidden;
		justify-content: space-between;
		white-space: nowrap;
		gap: 8px;
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
		gap: 4px;
	}

	.uncommitted-changes {
		display: flex;
		flex: 1;
		width: 100%;
		height: 100%;
		display: flex;
		justify-items: top;
		flex-direction: column;
		align-items: top;
		justify-content: top;
		overflow: hidden;
	}

	.start-commit {
		padding: 16px;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 20px;
		padding: 0 20px 40px;
		height: 100%;
	}

	.empty-state-text {
		text-align: center;
		color: var(--clr-text-3);
	}
</style>
