<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import BranchExplorer from '$components/v3/BranchExplorer.svelte';
	import BranchView from '$components/v3/BranchView.svelte';
	import BranchesViewBranch from '$components/v3/BranchesViewBranch.svelte';
	import BranchesViewStack from '$components/v3/BranchesViewStack.svelte';
	import SelectionView from '$components/v3/SelectionView.svelte';
	import TargetCommitList from '$components/v3/TargetCommitList.svelte';
	import UnappliedBranchView from '$components/v3/UnappliedBranchView.svelte';
	import UnappliedCommitView from '$components/v3/UnappliedCommitView.svelte';
	import BranchListCard from '$components/v3/branchesPage/BranchListCard.svelte';
	import BranchesListGroup from '$components/v3/branchesPage/BranchesListGroup.svelte';
	import CurrentOriginCard from '$components/v3/branchesPage/CurrentOriginCard.svelte';
	import PRListCard from '$components/v3/branchesPage/PRListCard.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { Focusable } from '$lib/focus/focusManager.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { SidebarEntrySubject } from '$lib/branches/branchListing';
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
	const branchesSelection = $derived(projectState.branchesSelection);

	let leftDiv = $state<HTMLElement>();
	let rightDiv = $state<HTMLElement>();

	const leftWidth = $derived(uiState.global.leftWidth);
	const rightWidth = $derived(uiState.global.stacksViewWidth);

	const selectionId: SelectionId | undefined = $derived.by(() => {
		const current = branchesState?.current;
		if (current.commitId) {
			return { type: 'commit', commitId: current.commitId };
		}
		if (current.branchName) {
			return {
				type: 'branch',
				branchName: current.remote ? current.remote + '/' + current.branchName : current.branchName,
				stackId: current.stackId
			};
		}
		return undefined;
	});

	$effect(() => {});
</script>

<ReduxResult {projectId} result={baseBranchResult.current}>
	{#snippet children(baseBranch)}
		{@const lastCommit = baseBranch.recentCommits.at(0)}
		{@const current = branchesState.current}
		<div class="branches" use:focusable={{ id: Focusable.Branches }}>
			<div class="branch-list" bind:this={leftDiv} style:width={leftWidth.current + 'rem'}>
				<BranchesListGroup title="Current workspace target">
					<!-- TODO: We need an API for `commitsCount`! -->
					<CurrentOriginCard
						originName={baseBranch.branchName}
						commitsAmount={13140}
						lastCommit={lastCommit
							? {
									author: lastCommit.author,
									ago: getTimeAgo(lastCommit.createdAt, true),
									branch: baseBranch.shortName,
									sha: lastCommit.id.slice(0, 7)
								}
							: undefined}
						onclick={() => {
							branchesSelection.set({ branchName: baseBranch.shortName, isTarget: true });
						}}
						selected={branchesSelection.current.branchName === baseBranch.shortName}
					/>
				</BranchesListGroup>
				<BranchExplorer {projectId}>
					{#snippet sidebarEntry(sidebarEntrySubject: SidebarEntrySubject)}
						{#if sidebarEntrySubject.type === 'branchListing'}
							<BranchListCard
								{projectId}
								branchListing={sidebarEntrySubject.subject}
								prs={sidebarEntrySubject.prs}
								selected={branchesSelection.current.branchName === sidebarEntrySubject.subject.name}
								onclick={({ listing, pr }) => {
									if (listing.stack) {
										branchesSelection.set({
											stackId: listing.stack.id,
											branchName: listing.stack.branches.at(0),
											prNumber: pr?.number,
											inWorkspace: listing.stack.inWorkspace,
											hasLocal: listing.hasLocal
										});
									} else {
										branchesSelection.set({
											branchName: listing.name,
											prNumber: pr?.number,
											remote: listing.remotes.at(0),
											hasLocal: listing.hasLocal
										});
									}
								}}
							/>
						{:else}
							<PRListCard
								{projectId}
								pullRequest={sidebarEntrySubject.subject}
								selected={branchesSelection.current.prNumber === sidebarEntrySubject.subject.number}
								onclick={(pr) => branchesSelection.set({ prNumber: pr.number })}
								noSourceBranch
							/>
						{/if}
					{/snippet}
				</BranchExplorer>
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
				{#if current.branchName && current.commitId}
					<UnappliedCommitView
						{projectId}
						branchName={current.branchName}
						commitId={current.commitId}
						remote={current.remote}
					/>
				{:else if current.branchName}
					{#if current.inWorkspace && current.stackId}
						<BranchView {projectId} branchName={current.branchName} stackId={current.stackId} />
					{:else if current.isTarget}
						<UnappliedBranchView
							{projectId}
							branchName={current.branchName}
							remote={current.remote}
							isTarget={current.isTarget}
						/>
					{:else}
						<UnappliedBranchView
							{projectId}
							branchName={current.branchName}
							stackId={current.stackId}
							remote={current.remote}
							prNumber={current.prNumber}
							hasLocal={current.hasLocal}
						/>
					{/if}
				{/if}
			</div>
			<div class="branch-details" bind:this={rightDiv} style:width={rightWidth.current + 'rem'}>
				{#if current.branchName === baseBranch.shortName}
					<TargetCommitList {projectId} />
				{:else if current.stackId}
					<BranchesViewStack {projectId} stackId={current.stackId} />
				{:else if current.branchName}
					<BranchesViewBranch {projectId} branchName={current.branchName} remote={current.remote} />
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
