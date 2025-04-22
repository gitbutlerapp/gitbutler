<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import BranchView from '$components/v3/BranchView.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import NewCommitView from '$components/v3/NewCommitView.svelte';
	import ReviewView from '$components/v3/ReviewView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import StackTabs from '$components/v3/stackTabs/StackTabs.svelte';
	import { Focusable, FocusManager } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { remToPx } from '@gitbutler/ui/utils/remToPx';
	import { type Snippet } from 'svelte';
	import type { SelectionId } from '$lib/selection/key';

	interface Props {
		projectId: string;
		stackId?: string;
		stack: Snippet;
	}

	const { stackId, projectId, stack }: Props = $props();

	const [stackService, uiState, focusManager] = inject(StackService, UiState, FocusManager);
	const stacksResult = $derived(stackService.stacks(projectId));

	const projectState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectState.drawerPage);
	const drawerIsFullScreen = $derived(projectState.drawerFullScreen);
	const isCommitting = $derived(drawerPage.current === 'new-commit');

	let focusGroup = $derived(
		focusManager.radioGroup({
			triggers: [Focusable.UncommittedChanges, Focusable.ChangedFiles]
		})
	);

	const stackSelection = $derived(stackId ? uiState.stack(stackId).selection : undefined);
	const currentSelection = $derived(stackSelection?.current);
	const branchName = $derived(currentSelection?.branchName);
	const commitId = $derived(currentSelection?.commitId);
	const upstream = $derived(!!currentSelection?.upstream);

	const selectionId: SelectionId = $derived.by(() => {
		if (focusGroup.current === Focusable.ChangedFiles && currentSelection && stackId) {
			if (currentSelection.commitId) {
				return {
					type: 'commit',
					commitId: currentSelection.commitId
				};
			}
			return {
				type: 'branch',
				branchName: currentSelection.branchName,
				stackId
			};
		}
		return { type: 'worktree' };
	});

	const leftWidth = $derived(uiState.global.leftWidth);
	const stacksViewWidth = $derived(uiState.global.stacksViewWidth);

	let leftDiv = $state<HTMLElement>();
	let stacksViewEl = $state<HTMLElement>();

	let tabsWidth = $state<number>();
</script>

<div class="workspace" use:focusable={{ id: Focusable.Workspace }}>
	<div
		class="changed-files-view"
		bind:this={leftDiv}
		style:width={leftWidth.current + 'rem'}
		use:focusable={{ id: Focusable.WorkspaceLeft, parentId: Focusable.Workspace }}
	>
		<WorktreeChanges {projectId} {stackId} />
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
			<SelectionView {projectId} {selectionId} />
		{/if}

		{#if drawerPage.current === 'new-commit'}
			<NewCommitView {projectId} {stackId} />
		{:else if drawerPage.current === 'branch' && stackId && branchName}
			<BranchView {stackId} {projectId} {branchName} />
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
			{#snippet children(stacks)}
				<StackTabs
					{projectId}
					{stacks}
					selectedId={stackId}
					{isCommitting}
					bind:width={tabsWidth}
				/>
				<div
					class="contents"
					class:rounded={tabsWidth! <= (remToPx(stacksViewWidth.current - 0.5) as number)}
					class:dotted={stacks.length > 0}
				>
					{@render stack()}
				</div>
			{/snippet}
		</ReduxResult>
		<Resizer
			viewport={stacksViewEl}
			direction="left"
			minWidth={16}
			borderRadius="ml"
			onWidth={(value) => {
				stacksViewWidth.current = value;
			}}
		/>
	</div>
</div>

<style>
	.workspace {
		display: flex;
		flex: 1;
		gap: 8px;
		align-items: stretch;
		height: 100%;
		width: 100%;
		position: relative;
	}

	.changed-files-view {
		height: 100%;
		display: flex;
		flex-direction: column;
		justify-content: flex-start;
		position: relative;
		background-color: var(--clr-bg-1);
		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		flex-shrink: 0;
		overflow: hidden;
	}

	.stacks-view-wrap {
		height: 100%;
		display: flex;
		flex-direction: column;
		justify-content: flex-start;
		position: relative;
		flex-shrink: 0;
	}

	.main-view {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		border-radius: var(--radius-ml);
		overflow-x: hidden;
		position: relative;
		gap: 8px;
		min-width: 600px;
	}

	.contents {
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;

		border-radius: 0 0 var(--radius-ml) var(--radius-ml);
		border: 1px solid var(--clr-border-2);
	}

	.dotted {
		background-image: radial-gradient(
			oklch(from var(--clr-scale-ntrl-50) l c h / 0.5) 0.6px,
			#ffffff00 0.6px
		);
		background-size: 6px 6px;
	}

	.rounded {
		border-radius: 0 var(--radius-ml) var(--radius-ml) var(--radius-ml);
	}
</style>
