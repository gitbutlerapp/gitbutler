<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchView from '$components/v3/BranchView.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import MainViewport from '$components/v3/MainViewport.svelte';
	import MultiStackView from '$components/v3/MultiStackView.svelte';
	import NewCommitView from '$components/v3/NewCommitView.svelte';
	import ReviewView from '$components/v3/ReviewView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import WorktreeTipsFooter from '$components/v3/WorktreeTipsFooter.svelte';
	import noChanges from '$lib/assets/illustrations/no-changes.svg?raw';
	import { threePointFive } from '$lib/config/uiFeatureFlags';
	import { isParsedError } from '$lib/error/parser';
	import {
		DefinedFocusable,
		FocusManager,
		parseSnapshotChangesFocusableId,
		parseFocusableId,
		stackFocusableId,
		uncommittedFocusableId
	} from '$lib/focus/focusManager.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		stackId?: string;
	}

	const { stackId, projectId }: Props = $props();

	const [stackService, uiState, focusManager, idSelection] = inject(
		StackService,
		UiState,
		FocusManager,
		IdSelection
	);
	const worktreeSelection = idSelection.getById({ type: 'worktree' });
	const stacksResult = $derived(stackService.stacks(projectId));

	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage);
	const drawerIsFullScreen = $derived(projectState.drawerFullScreen);

	const snapshotFocusables = writable<string[]>([]);
	setContext('snapshot-focusables', snapshotFocusables);

	const stackFocusables = $derived(
		stacksResult.current?.data
			? stacksResult.current.data.map((stack) => stackFocusableId(stack.id))
			: []
	);

	const uncommittedFocusables = $derived(
		stacksResult.current?.data
			? stacksResult.current.data.map((stack) => uncommittedFocusableId(stack.id))
			: []
	);

	let focusGroup = $derived(
		focusManager.radioGroup({
			triggers: [
				DefinedFocusable.UncommittedChanges,
				DefinedFocusable.Drawer,
				...stackFocusables,
				...$snapshotFocusables,
				...uncommittedFocusables
			]
		})
	);

	const stackSelection = $derived(stackId ? uiState.stack(stackId).selection : undefined);
	const currentSelection = $derived(stackSelection?.current);
	const branchName = $derived(currentSelection?.branchName);
	const commitId = $derived(currentSelection?.commitId);
	const upstream = $derived(!!currentSelection?.upstream);

	const focusedStackId = $derived(
		focusGroup.current ? parseFocusableId(focusGroup.current) : undefined
	);

	const selectionId: SelectionId = $derived.by(() => {
		if ($threePointFive) return { type: 'worktree', stackId: undefined };
		const branchName = currentSelection?.branchName;

		if (focusGroup.current?.startsWith(DefinedFocusable.UncommittedChanges)) {
			return { type: 'worktree', stackId: focusedStackId };
		}

		const snapshot = focusGroup.current
			? parseSnapshotChangesFocusableId(focusGroup.current)
			: undefined;
		if (snapshot) {
			return { type: 'snapshot', before: snapshot.before, after: snapshot.after };
		}

		if (
			focusGroup.current === DefinedFocusable.UncommittedChanges &&
			worktreeSelection.entries.size > 0
		) {
			return { type: 'worktree', stackId: undefined };
		}
		if (currentSelection && stackId && branchName) {
			if (currentSelection.commitId) {
				const selectionId = { type: 'commit', commitId: currentSelection.commitId } as const;
				if (idSelection.hasItems(selectionId)) return selectionId;
			}
			const selectionId = { type: 'branch', stackId: stackId, branchName } as const;
			if (idSelection.hasItems(selectionId)) return selectionId;
		}
		return { type: 'worktree', stackId: focusedStackId };
	});

	function onerror(err: unknown) {
		// Clear selection if branch not found.
		if (isParsedError(err) && err.code === 'errors.branch.notfound') {
			stackSelection?.set(undefined);
			console.warn('Workspace selection cleared');
		}
	}

	const lastAdded = $derived(worktreeSelection.lastAdded);
	const previewOpen = $derived(!!$lastAdded?.key);
</script>

<MainViewport
	name="workspace"
	middleOpen={previewOpen}
	leftWidth={{ default: 280, min: 240 }}
	middleWidth={{ default: 380, min: 240 }}
>
	{#snippet left()}
		<WorktreeChanges
			title="Unassigned"
			{projectId}
			stackId={undefined}
			active={selectionId.type === 'worktree' &&
				selectionId.stackId === undefined &&
				(!$threePointFive || focusGroup.current === DefinedFocusable.UncommittedChanges)}
		>
			{#snippet emptyPlaceholder()}
				<div class="unassigned-changes__empty">
					<div class="unassigned-changes__empty__placeholder">
						{@html noChanges}
						<p class="text-13 text-body unassigned-changes__empty__placeholder-text">
							You're all caught up!<br />
							No files need committing
						</p>
					</div>
					<WorktreeTipsFooter />
				</div>
			{/snippet}
		</WorktreeChanges>
	{/snippet}
	{#snippet middle()}
		{#if !drawerIsFullScreen.current || $threePointFive}
			<SelectionView {projectId} {selectionId} draggableFiles />
		{/if}
		{#if !$threePointFive}
			{#if drawerPage.current === 'new-commit'}
				<NewCommitView {projectId} />
			{:else if drawerPage.current === 'branch' && stackId && branchName}
				<BranchView
					{stackId}
					{projectId}
					{branchName}
					{onerror}
					active={selectionId.type !== 'worktree'}
					draggableFiles
				/>
			{:else if drawerPage.current === 'review' && stackId && branchName}
				<ReviewView {stackId} {projectId} {branchName} />
			{:else if branchName && commitId && stackId}
				<CommitView
					{projectId}
					{stackId}
					commitKey={{
						stackId,
						branchName,
						commitId,
						upstream
					}}
					active={selectionId.type !== 'worktree'}
					{onerror}
				/>
			{/if}
		{/if}
	{/snippet}
	{#snippet right()}
		<ReduxResult {projectId} result={stacksResult?.current}>
			{#snippet loading()}
				<div class="stacks-view-skeleton"></div>
			{/snippet}

			{#snippet children(stacks)}
				<MultiStackView {projectId} {stacks} {selectionId} selectedId={stackId} {focusedStackId} />
			{/snippet}
		</ReduxResult>
	{/snippet}
</MainViewport>

<style>
	.stacks-view-skeleton {
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.unassigned-changes__empty {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.unassigned-changes__empty__placeholder {
		display: flex;
		flex: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 0 20px 40px;
		gap: 20px;
	}

	.unassigned-changes__empty__placeholder-text {
		color: var(--clr-text-3);
		text-align: center;
	}
</style>
