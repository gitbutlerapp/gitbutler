<script lang="ts">
	import Resizer from '$components/Resizer.svelte';
	import BranchView from '$components/v3/BranchView.svelte';
	import CommitView from '$components/v3/CommitView.svelte';
	import NewButlerReview from '$components/v3/NewButlerReview.svelte';
	import NewCommit from '$components/v3/NewCommit.svelte';
	import NewPullRequest from '$components/v3/NewPullRequest.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
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
	const drawerPage = $derived(projectUiState.drawerPage.get());
	const drawerIsFullScreen = $derived(projectUiState.drawerFullScreen.get());
	const selected = $derived(uiState.stack(stackId!).selection.get());
	const branchName = $derived(selected.current?.branchName);

	const leftWidth = $state(uiState.global.leftWidth.get());
	const rightWidth = $state(uiState.global.rightWidth.get());

	let leftDiv = $state<HTMLElement>();
	let rightDiv = $state<HTMLElement>();
</script>

<div class="workspace">
	<div class="changed-files-view" bind:this={leftDiv} style:width={leftWidth.current + 'rem'}>
		<WorktreeChanges {projectId} {stackId} />
		<Resizer
			viewport={leftDiv}
			direction="right"
			minWidth={14}
			onWidth={(value) => uiState.global.leftWidth.set(value)}
		/>
	</div>
	<div class="main-view">
		{#if !drawerIsFullScreen.current}
			<SelectionView {projectId} />
		{/if}

		{#if stackId}
			{#if drawerPage.current === 'new-commit'}
				<NewCommit {projectId} {stackId} />
			{:else if drawerPage.current === 'branch' && branchName}
				<BranchView {stackId} {projectId} {branchName} />
			{:else if drawerPage.current === 'pr'}
				<NewPullRequest {stackId} {projectId} />
			{:else if drawerPage.current === 'br'}
				<NewButlerReview {stackId} {projectId} />
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

	<div class="right" bind:this={rightDiv} style:width={rightWidth.current + 'rem'}>
		{@render right({ viewportWidth: rightWidth.current })}
		<Resizer
			viewport={rightDiv}
			direction="left"
			minWidth={16}
			onWidth={(value) => uiState.global.rightWidth.set(value)}
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
		/* Resizer looks better with hidden overflow. */
		overflow: hidden;
		flex-shrink: 0;
	}

	.right {
		height: 100%;
		display: flex;
		flex-direction: column;
		justify-content: flex-start;
		position: relative;
		/* Resizer looks better with hidden overflow. */
		overflow: hidden;
		flex-shrink: 0;
	}

	.main-view {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		border-radius: var(--radius-ml);
		gap: 10px;
	}
</style>
