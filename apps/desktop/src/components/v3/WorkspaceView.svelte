<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import BranchView from '$components/v3/BranchView.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import MultiStackView from '$components/v3/MultiStackView.svelte';
	import NewCommitView from '$components/v3/NewCommitView.svelte';
	import ReviewView from '$components/v3/ReviewView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import { isParsedError } from '$lib/error/parser';
	import { Focusable, FocusManager } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
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

	let focusGroup = $derived(
		focusManager.radioGroup({
			triggers: [Focusable.UncommittedChanges, Focusable.Drawer, Focusable.WorkspaceRight]
		})
	);

	const stackSelection = $derived(stackId ? uiState.stack(stackId).selection : undefined);
	const currentSelection = $derived(stackSelection?.current);
	const branchName = $derived(currentSelection?.branchName);
	const commitId = $derived(currentSelection?.commitId);
	const upstream = $derived(!!currentSelection?.upstream);

	const selectionId: SelectionId = $derived.by(() => {
		const branchName = currentSelection?.branchName;
		if (focusGroup.current === Focusable.UncommittedChanges && worktreeSelection.entries.size > 0) {
			return { type: 'worktree' };
		}
		if (currentSelection && stackId && branchName) {
			if (currentSelection.commitId) {
				const selectionId = { type: 'commit', commitId: currentSelection.commitId } as const;
				if (idSelection.hasItems(selectionId)) return selectionId;
			}
			const selectionId = { type: 'branch', stackId: stackId, branchName } as const;
			if (idSelection.hasItems(selectionId)) return selectionId;
		}

		return { type: 'worktree' };
	});

	const leftWidth = $derived(uiState.global.leftWidth);
	const stacksViewWidth = $derived(uiState.global.stacksViewWidth);

	let leftDiv = $state<HTMLElement>();
	let stacksViewEl = $state<HTMLElement>();

	function onerror(err: unknown) {
		// Clear selection if branch not found.
		if (isParsedError(err) && err.code === 'errors.branch.notfound') {
			stackSelection?.set(undefined);
			console.warn('Workspace selection cleared');
		}
	}
</script>

<div class="workspace" use:focusable={{ id: Focusable.Workspace }}>
	<div
		class="changed-files-view"
		bind:this={leftDiv}
		style:width={leftWidth.current + 'rem'}
		use:focusable={{ id: Focusable.WorkspaceLeft, parentId: Focusable.Workspace }}
	>
		<WorktreeChanges {projectId} {stackId} active={selectionId.type === 'worktree'} />
		<Resizer
			viewport={leftDiv}
			direction="right"
			minWidth={14}
			borderRadius="ml"
			onWidth={(value) => (leftWidth.current = value)}
		/>
	</div>
	<div
		class="main-view"
		use:focusable={{ id: Focusable.WorkspaceMiddle, parentId: Focusable.Workspace }}
	>
		{#if !drawerIsFullScreen.current}
			<SelectionView {projectId} {selectionId} {stackId} />
		{/if}

		{#if drawerPage.current === 'new-commit'}
			<NewCommitView {projectId} {stackId} />
		{:else if drawerPage.current === 'branch' && stackId && branchName}
			<BranchView
				{stackId}
				{projectId}
				{branchName}
				{onerror}
				active={selectionId.type !== 'worktree'}
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
	</div>

	<div
		class="stacks-view-wrap"
		bind:this={stacksViewEl}
		style:width={stacksViewWidth.current + 'rem'}
		use:focusable={{ id: Focusable.WorkspaceRight, parentId: Focusable.Workspace }}
	>
		<ReduxResult {projectId} result={stacksResult?.current}>
			{#snippet loading()}
				<div class="stacks-view-skeleton"></div>
			{/snippet}

			{#snippet children(stacks)}
				<MultiStackView
					{projectId}
					{stacks}
					selectedId={stackId}
					active={focusGroup.current !== Focusable.UncommittedChanges}
				/>
			{/snippet}
		</ReduxResult>
		<Resizer
			viewport={stacksViewEl}
			direction="left"
			minWidth={16}
			borderRadius="ml"
			onWidth={(value) => uiState.global.stacksViewWidth.set(value)}
		/>
	</div>
</div>

<style>
	.workspace {
		display: flex;
		position: relative;
		flex: 1;
		align-items: stretch;
		width: 100%;
		height: 100%;
		overflow: hidden;
		gap: 8px;
	}

	.changed-files-view {
		display: flex;
		position: relative;
		flex-shrink: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
	}

	.stacks-view-wrap {
		display: flex;
		position: relative;
		flex-grow: 0;
		flex-direction: column;
		justify-content: flex-start;
		height: 100%;
		overflow: hidden;
	}

	.main-view {
		display: flex;
		position: relative;
		flex-grow: 1;
		flex-direction: column;
		min-width: 320px;
		overflow-x: hidden;
		gap: 8px;
		border-radius: var(--radius-ml);
	}

	/* SKELETON LOADING */
	.stacks-view-skeleton {
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}
</style>
