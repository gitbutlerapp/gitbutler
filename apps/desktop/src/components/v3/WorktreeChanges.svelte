<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import FileListMode from '$components/v3/FileListMode.svelte';
	import WorktreeChangesFileList from '$components/v3/WorktreeChangesFileList.svelte';
	import WorktreeChangesSelectAll from '$components/v3/WorktreeChangesSelectAll.svelte';
	import WorktreeTipsFooter from '$components/v3/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { createCommitStore } from '$lib/commits/contexts';
	import { UncommitDzHandler } from '$lib/commits/dropHandler';
	import { DefinedFocusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { previousPathBytesFromTreeChange } from '$lib/hunks/change';
	import { DiffService } from '$lib/hunks/diffService.svelte';
	import { AssignmentDropHandler } from '$lib/hunks/dropHandler';
	import { ChangeSelectionService, type SelectedFile } from '$lib/selection/changeSelection.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { WorktreeService } from '$lib/worktree/worktreeService.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { untrack } from 'svelte';

	type Props = {
		projectId: string;
		stackId?: string;
		active: boolean;
	};

	let { projectId, stackId, active }: Props = $props();

	const [changeSelection, worktreeService, uiState, stackService, idSelection] = inject(
		ChangeSelectionService,
		WorktreeService,
		UiState,
		StackService,
		IdSelection
	);

	const uncommitDzHandler = $derived(new UncommitDzHandler(projectId, stackService, uiState));

	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage.get());
	const isCommitting = $derived(drawerPage.current === 'new-commit');
	const stackState = $derived(stackId ? uiState.stack(stackId) : undefined);

	const defaultBranchResult = $derived(
		stackId !== undefined ? stackService.defaultBranch(projectId, stackId) : undefined
	);
	const defaultBranchName = $derived(defaultBranchResult?.current.data);

	const selectedChanges = changeSelection.list();
	const noChangesSelected = $derived(selectedChanges.current.length === 0);
	const changesResult = $derived(worktreeService.getChanges(projectId));
	const affectedPaths = $derived(changesResult.current.data?.map((c) => c.path));

	// TODO: Make this go away.
	createCommitStore(undefined);

	/** Clear any selected changes that no longer exist. */
	$effect(() => {
		if (affectedPaths) {
			untrack(() => {
				changeSelection.retain(affectedPaths);
				// TODO: We need to make this consider all groups
				idSelection.retain(affectedPaths, { type: 'ungrouped' });
			});
		}
	});

	let listMode: 'list' | 'tree' = $state('list');

	function selectEverything() {
		const affectedPaths =
			changesResult.current.data?.map(
				(c) => [c.path, c.pathBytes, previousPathBytesFromTreeChange(c)] as const
			) ?? [];
		const files: SelectedFile[] = affectedPaths.map(([path, pathBytes, previousPathBytes]) => ({
			path,
			pathBytes,
			previousPathBytes,
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

	let listHeaderHeight = $state(0);
	let listFooterHeight = $state(0);

	const diffService = getContext(DiffService);

	const changesKeyResult = $derived(worktreeService.getChangesKey(projectId));
	const hunkAssignments = $derived(
		changesKeyResult.current
			? diffService.hunkAssignments(projectId, changesKeyResult.current)
			: undefined
	);
	const assignmentDZHandler = $derived(
		hunkAssignments?.current?.data
			? new AssignmentDropHandler(projectId, diffService, hunkAssignments.current.data, {
					type: 'ungrouped'
				})
			: undefined
	);
</script>

<Dropzone handlers={[uncommitDzHandler, assignmentDZHandler].filter(isDefined)} maxHeight>
	{#snippet overlay({ hovered, activated, handler })}
		<CardOverlay
			{hovered}
			{activated}
			label={handler instanceof UncommitDzHandler ? 'Uncommit changes' : 'Unassign changes'}
		/>
	{/snippet}

	<div
		class="uncommitted-changes-wrap"
		use:focusable={{
			id: DefinedFocusable.UncommittedChanges,
			parentId: DefinedFocusable.ViewportLeft
		}}
	>
		<ScrollableContainer
			autoScroll={false}
			padding={{
				top: listHeaderHeight,
				bottom: listFooterHeight
			}}
		>
			<ReduxResult {stackId} {projectId} result={changesResult.current}>
				{#snippet children(changes, { projectId })}
					<div
						data-testid={TestId.UncommittedChanges_Header}
						use:stickyHeader
						class="worktree-header"
						bind:clientHeight={listHeaderHeight}
					>
						<div class="worktree-header__general">
							{#if isCommitting}
								<WorktreeChangesSelectAll {projectId} group={{ type: 'ungrouped' }} />
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
						<div data-testid={TestId.UncommittedChanges_FileList} class="uncommitted-changes">
							<WorktreeChangesFileList
								draggableFiles
								{projectId}
								{listMode}
								{active}
								group={{ type: 'ungrouped' }}
							/>
						</div>
						<div
							use:stickyHeader={{ align: 'bottom' }}
							class="start-commit"
							bind:clientHeight={listFooterHeight}
						>
							<Button
								testId={TestId.StartCommitButton}
								kind={isCommitting ? 'outline' : 'solid'}
								type="button"
								size="cta"
								wide
								disabled={isCommitting || defaultBranchResult?.current.isLoading}
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
				{/snippet}
			</ReduxResult>
		</ScrollableContainer>
	</div>
</Dropzone>

<style>
	.uncommitted-changes-wrap {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
	}

	.worktree-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 10px 10px 10px 14px;
		gap: 8px;
		background-color: var(--clr-bg-1);
		text-wrap: nowrap;
		white-space: nowrap;
	}

	.worktree-header__general {
		display: flex;
		align-items: center;
		overflow: hidden;
		gap: 10px;
	}

	.worktree-header__title {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.uncommitted-changes {
		display: flex;
		flex: 1;
		flex-direction: column;
		width: 100%;
	}

	.start-commit {
		position: sticky;
		bottom: -1px;
		padding: 14px;
		background-color: var(--clr-bg-1);
	}

	.uncommitted-changes__empty {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.uncommitted-changes__empty__placeholder {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 0 20px 40px;
		gap: 20px;
	}

	.uncommitted-changes__empty__placeholder-text {
		color: var(--clr-text-3);
		text-align: center;
	}
</style>
