<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchExplorer from '$components/v3/BranchExplorer.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import GitCommitView from '$components/v3/GitCommitView.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import BranchesListGroup from '$components/v3/branchesPage/BranchesListGroup.svelte';
	import CurrentOriginCard from '$components/v3/branchesPage/CurrentOriginCard.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { Focusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { SelectionId } from '$lib/selection/key';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const [uiState, baseBranchService] = inject(UiState, BaseBranchService);

	const projectState = $derived(uiState.project(projectId));
	const branchesState = $derived(projectState.branchesSelection);
	const drawerIsFullScreen = $derived(projectState.drawerFullScreen);
	const baseBranchResult = $derived(baseBranchService.baseBranch(projectId));

	let leftDiv = $state<HTMLElement>();
	let rightDiv = $state<HTMLElement>();

	const leftWidth = $derived(uiState.global.leftWidth);
	const rightWidth = $derived(uiState.global.stacksViewWidth);

	const selectionId: SelectionId | undefined = $derived.by(() => {
		const current = branchesState?.current;
		if (current.commitId) {
			return { type: 'commit', commitId: current.commitId };
		}
		return undefined;
	});
</script>

<ReduxResult {projectId} result={baseBranchResult.current}>
	{#snippet children(baseBranch)}
		{@const lastCommit = baseBranch.recentCommits.at(0)}
		{@const current = branchesState.current};
		<div class="branches" use:focusable={{ id: Focusable.Branches }}>
			<div class="branch-list" bind:this={leftDiv} style:width={leftWidth.current + 'rem'}>
				<BranchesListGroup title="Current workspace target">
					<!-- TODO: We need an API for `commitsCount`! -->
					<CurrentOriginCard
						originName="origin/master"
						commitsAmount={13140}
						lastCommit={lastCommit
							? {
									author: lastCommit.author.name || lastCommit.author.email,
									ago: getTimeAgo(lastCommit.createdAt, true),
									branch: baseBranch.shortName,
									sha: lastCommit.id.slice(0, 7)
								}
							: undefined}
						onclick={() => {
							projectState.branchesSelection.set({ branchName: baseBranch.branchName });
						}}
					/>
				</BranchesListGroup>
				<BranchExplorer {projectId} />
				<Resizer
					viewport={leftDiv}
					direction="right"
					minWidth={14}
					borderRadius="ml"
					onWidth={(value) => (leftWidth.current = value)}
				/>
			</div>
			<div class="main-view">
				{#if !drawerIsFullScreen.current}
					<SelectionView {projectId} {selectionId} />
				{/if}
				{#if current.commitId}
					<GitCommitView {projectId} commitId={current.commitId} commitMessage="" />
				{/if}
			</div>
			<div class="branch-details" bind:this={rightDiv} style:width={rightWidth.current + 'rem'}>
				{#if current.branchName === baseBranch.branchName}
					<ReduxResult {projectId} result={baseBranchResult.current}>
						{#snippet children(baseBranch)}
							{@const branchName = baseBranch.branchName}
							<BranchCard type="normal-branch" {projectId} branchName={baseBranch.branchName}>
								{#snippet header()}
									<BranchHeader
										type="normal-branch"
										{branchName}
										{projectId}
										lineColor={getColorFromBranchType('LocalOnly')}
										iconName="branch-upstream"
										lastUpdatedAt={baseBranch.recentCommits.at(0)?.createdAt.getTime()}
										readonly
										onclick={() => {
											uiState.project(projectId).branchesSelection.set({
												branchName
											});
										}}
									></BranchHeader>
								{/snippet}
								{#snippet commitList()}
									{#each baseBranch.recentCommits as commit}
										<CommitRow
											type="Base"
											commitMessage={commit.description}
											createdAt={commit.createdAt.getTime()}
											commitId={commit.id}
											{projectId}
											branchName={baseBranch.shortName}
											selected={commit.id === branchesState?.current.commitId}
											onclick={() => {
												branchesState.set({
													commitId: commit.id,
													branchName: baseBranch.branchName
												});
											}}
										/>
									{/each}
								{/snippet}
							</BranchCard>
						{/snippet}
					</ReduxResult>
				{:else if current.branchName}
					<BranchCard type="normal-branch" {projectId} branchName={current.branchName}>
						{#snippet commitList()}
							Not implemented!
						{/snippet}
					</BranchCard>
				{:else if current.prNumber}
					Not implemented!
				{/if}
				<Resizer
					viewport={rightDiv}
					direction="left"
					minWidth={16}
					borderRadius="ml"
					onWidth={(value) => {
						rightWidth.current = value;
					}}
				/>
			</div>
		</div>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.branches {
		display: flex;
		flex: 1;
		gap: 8px;
		align-items: stretch;
		height: 100%;
		width: 100%;
		position: relative;
	}
	.branch-list {
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
	.branch-details {
		height: 100%;
		display: flex;
		flex-direction: column;
		justify-content: flex-start;
		position: relative;
		flex-shrink: 0;
	}
</style>
