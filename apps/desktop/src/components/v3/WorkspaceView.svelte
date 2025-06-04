<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import ActionLog from '$components/v3/ActionLog.svelte';
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
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { isParsedError } from '$lib/error/parser';
	import {
		assignedChangesFocusableId,
		DefinedFocusable,
		FocusManager,
		parseSnapshotChangesFocusableId,
		parseUnassignedChangesFocusable
	} from '$lib/focus/focusManager.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import Segment from '@gitbutler/ui/segmentControl/Segment.svelte';
	import SegmentControl from '@gitbutler/ui/segmentControl/SegmentControl.svelte';
	import { setContext } from 'svelte';
	import { writable } from 'svelte/store';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		stackId?: string;
	}

	const { stackId, projectId }: Props = $props();

	const [stackService, uiState, focusManager, idSelection, settingsService] = inject(
		StackService,
		UiState,
		FocusManager,
		IdSelection,
		SettingsService
	);
	const worktreeSelection = idSelection.getById({ type: 'worktree' });
	const stacksResult = $derived(stackService.stacks(projectId));
	const settingsStore = $derived(settingsService.appSettings);
	const canUseActions = $derived($settingsStore?.featureFlags.actions ?? false);

	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage);
	const drawerIsFullScreen = $derived(projectState.drawerFullScreen);

	const snapshotFocusables = writable<string[]>([]);
	setContext('snapshot-focusables', snapshotFocusables);

	const stackFocusables = $derived(
		stacksResult.current?.data
			? stacksResult.current.data.map((stack) => assignedChangesFocusableId(stack.id))
			: []
	);

	let focusGroup = $derived(
		focusManager.radioGroup({
			triggers: [
				DefinedFocusable.UncommittedChanges,
				DefinedFocusable.Drawer,
				DefinedFocusable.ViewportRight,
				...stackFocusables,
				...$snapshotFocusables
			]
		})
	);

	const stackSelection = $derived(stackId ? uiState.stack(stackId).selection : undefined);
	const currentSelection = $derived(stackSelection?.current);
	const branchName = $derived(currentSelection?.branchName);
	const commitId = $derived(currentSelection?.commitId);
	const upstream = $derived(!!currentSelection?.upstream);

	const selectionId: SelectionId = $derived.by(() => {
		const branchName = currentSelection?.branchName;
		const assignedChangesStackId = focusGroup.current
			? parseUnassignedChangesFocusable(focusGroup.current)
			: undefined;
		if (assignedChangesStackId) {
			return { type: 'worktree', stackId: assignedChangesStackId };
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

		return { type: 'worktree' };
	});

	const view = persisted<'worktree' | 'action-log'>('worktree', 'left-sidebar-tab');

	function onerror(err: unknown) {
		// Clear selection if branch not found.
		if (isParsedError(err) && err.code === 'errors.branch.notfound') {
			stackSelection?.set(undefined);
			console.warn('Workspace selection cleared');
		}
	}
</script>

<MainViewport
	name="workspace"
	leftWidth={{ default: 280, min: 240 }}
	rightWidth={{ default: 380, min: 240 }}
>
	{#snippet left()}
		{#if canUseActions}
			<div class="left-view-toggle">
				<SegmentControl
					fullWidth
					defaultIndex={$view === 'worktree' ? 0 : 1}
					onselect={(id) => {
						$view = id as 'worktree' | 'action-log';
					}}
				>
					<Segment id="worktree" icon="file-changes" />
					<Segment id="action-log" icon="ai" />
				</SegmentControl>
			</div>
		{/if}

		{#if !canUseActions || $view === 'worktree'}
			<div class="unassigned-changes__container">
				<WorktreeChanges
					title="Unassigned"
					{projectId}
					stackId={undefined}
					active={selectionId.type === 'worktree' && selectionId.stackId === undefined}
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
			</div>
		{:else if canUseActions && $view === 'action-log'}
			<ActionLog {projectId} {selectionId} />
		{/if}
	{/snippet}

	{#snippet middle()}
		{#if !drawerIsFullScreen.current}
			<SelectionView {projectId} {selectionId} draggableFiles />
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
	{/snippet}

	{#snippet right()}
		<ReduxResult {projectId} result={stacksResult?.current}>
			{#snippet loading()}
				<div class="stacks-view-skeleton"></div>
			{/snippet}

			{#snippet children(stacks)}
				<MultiStackView
					{projectId}
					{stacks}
					{selectionId}
					selectedId={stackId}
					active={focusGroup.current !== DefinedFocusable.UncommittedChanges}
				/>
			{/snippet}
		</ReduxResult>
	{/snippet}
</MainViewport>

<style>
	/* SKELETON LOADING */
	.stacks-view-skeleton {
		width: 100%;
		height: 100%;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.left-view-toggle {
		display: flex;
		align-items: center;
		justify-content: center;
		margin-bottom: 8px;
	}

	/* UNASSIGN CHANGES */
	.unassigned-changes__container {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
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
