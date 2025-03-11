<script lang="ts">
	import BranchView from './BranchView.svelte';
	import CommitView from './CommitView.svelte';
	import NewButlerReview from './NewButlerReview.svelte';
	import NewCommit from './NewCommit.svelte';
	import NewPullRequest from './NewPullRequest.svelte';
	import SelectionView from './SelectionView.svelte';
	import Resizer from '$components/Resizer.svelte';
	import WorktreeChanges from '$components/v3/WorktreeChanges.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { type Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId?: string;
		right: Snippet<[{ viewportWidth: number }]>;
	}

	const { stackId, projectId, right }: Props = $props();

	const [uiState, idSelection] = inject(UiState, IdSelection);
	const drawerPage = $derived(uiState.project(projectId).drawerPage.get());
	const selected = $derived(uiState.stack(stackId!).selection.get());
	const branchName = $derived(selected.current?.branchName);
	const hasFileSelection = $derived(idSelection.length > 0);

	const leftWidth = $state(uiState.global.leftWidth.get());
	const rightWidth = $state(uiState.global.rightWidth.get());

	let leftDiv = $state<HTMLElement>();
	let rightDiv = $state<HTMLElement>();

	let height = $derived(uiState.global.drawerHeight.get());
	let drawerDiv: HTMLDivElement | undefined = $state();
</script>

<div class="workspace-view">
	<div class="left" bind:this={leftDiv} style:width={leftWidth.current + 'rem'}>
		<WorktreeChanges {projectId} />
		<Resizer
			viewport={leftDiv}
			direction="right"
			minWidth={14}
			onWidth={(value) => uiState.global.leftWidth.set(value)}
		/>
	</div>
	<div class="middle">
		{#if hasFileSelection}
			<SelectionView {projectId} />
		{/if}
		<div
			class="drawer"
			bind:this={drawerDiv}
			style:height={hasFileSelection ? 'min(' + height.current + 'rem, 80%)' : undefined}
			style:flex-grow={hasFileSelection ? undefined : '1'}
		>
			{#if stackId}
				{#if drawerPage.current === 'new-commit'}
					<NewCommit {projectId} {stackId} />
				{:else if drawerPage.current === 'branch'}
					<BranchView {stackId} {projectId} {branchName} />
				{:else if drawerPage.current === 'pr'}
					<NewPullRequest {stackId} {projectId} />
				{:else if drawerPage.current === 'br'}
					<NewButlerReview {stackId} {projectId} />
				{:else if selected.current?.branchName && selected.current.commitId}
					<CommitView
						{projectId}
						commitKey={{
							stackId,
							branchName: selected.current.branchName,
							commitId: selected.current.commitId,
							upstream: !!selected.current.upstream
						}}
					/>
				{/if}
			{/if}
			{#if hasFileSelection}
				<Resizer
					direction="up"
					viewport={drawerDiv}
					minHeight={11}
					onHeight={(value) => uiState.global.drawerHeight.set(value)}
				/>
			{/if}
		</div>
	</div>
	<div class="right" bind:this={rightDiv} style:width={rightWidth.current + 'rem'}>
		{@render right({ viewportWidth: rightWidth.current })}
		<Resizer
			viewport={rightDiv}
			direction="left"
			minWidth={14}
			onWidth={(value) => uiState.global.rightWidth.set(value)}
		/>
	</div>
</div>

<style>
	.workspace-view {
		display: flex;
		flex: 1;
		gap: 14px;
		align-items: stretch;
		height: 100%;
		width: 100%;
		position: relative;
	}

	.left {
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

	.middle {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		border-radius: var(--radius-ml);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;
	}

	.drawer {
		position: relative;
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
	}
</style>
