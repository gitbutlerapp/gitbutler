<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import BranchView from '$components/v3/BranchView.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import NewCommitView from '$components/v3/NewCommitView.svelte';
	import ReviewView from '$components/v3/ReviewView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { type Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId?: string;
		right: Snippet<[{ viewportWidth: number }]>;
	}

	const { stackId, projectId, right }: Props = $props();

	const [uiState] = inject(UiState);
	const projectUiState = $derived(uiState.project(projectId));
	const drawerPage = $derived(projectUiState.drawerPage);
	const drawerIsFullScreen = $derived(projectUiState.drawerFullScreen);
	const selected = $derived(uiState.stack(stackId!).selection);
	const branchName = $derived(selected.current?.branchName);

	const leftWidth = $derived(uiState.global.leftWidth);
	const stacksViewWidth = $derived(uiState.global.stacksViewWidth);

	let leftDiv = $state<HTMLElement>();
	let stacksViewEl = $state<HTMLElement>();
</script>

<div class="workspace" use:focusable={{ id: 'workspace' }}>
	<div
		class="changed-files-view"
		bind:this={leftDiv}
		style:width={leftWidth.current + 'rem'}
		use:focusable={{ id: 'left', parentId: 'workspace' }}
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
	<div class="main-view" use:focusable={{ id: 'main', parentId: 'workspace' }}>
		{#if !drawerIsFullScreen.current}
			<SelectionView {projectId} {stackId} />
		{/if}

		{#if stackId}
			{#if drawerPage.current === 'new-commit'}
				<NewCommitView {projectId} {stackId} />
			{:else if drawerPage.current === 'branch' && branchName}
				<BranchView {stackId} {projectId} {branchName} />
			{:else if drawerPage.current === 'review' && branchName}
				<ReviewView {stackId} {projectId} {branchName} />
			{:else if selected.current?.branchName && selected.current.commitId && stackId}
				<CommitView
					{projectId}
					{stackId}
					commitKey={{
						stackId,
						branchName: selected.current.branchName,
						commitId: selected.current.commitId,
						upstream: !!selected.current.upstream
					}}
				/>
			{/if}
		{/if}
	</div>

	<div
		class="stacks-view-wrap"
		bind:this={stacksViewEl}
		style:width={stacksViewWidth.current + 'rem'}
		use:focusable={{ id: 'right', parentId: 'workspace' }}
	>
		{@render right({ viewportWidth: stacksViewWidth.current })}
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
		min-width: 320px;
	}
</style>
