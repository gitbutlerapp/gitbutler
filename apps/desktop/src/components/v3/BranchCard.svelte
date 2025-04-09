<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import SeriesHeaderContextMenu from '$components/SeriesHeaderContextMenu.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import BranchDividerLine from '$components/v3/BranchDividerLine.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import NewBranchModal from '$components/v3/NewBranchModal.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { Snippet } from 'svelte';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		first: boolean;
		last: boolean;
		commitList: Snippet;
	}

	let { projectId, stackId, branchName, first, commitList }: Props = $props();

	const [stackService, uiState, forge] = inject(StackService, UiState, DefaultForgeFactory);

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const branchDetailsResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const commitResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0));
	const localAndRemoteCommits = $derived(stackService.commits(projectId, stackId, branchName));
	const upstreamOnlyCommits = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName)
	);

	const selection = $derived(uiState.stack(stackId).selection.get());
	const remoteBranchName = $derived(branchResult.current.data?.remoteTrackingBranch);

	const forgeBranch = $derived(
		remoteBranchName ? forge.current?.branch(remoteBranchName) : undefined
	);

	let headerEl = $state<HTMLDivElement>();
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabContextMenuTrigger = $state<HTMLButtonElement>();

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();
	let isContextMenuOpenedByBtn = $state(false);
	let isContextMenuOpenedByMouse = $state(false);
</script>

<ReduxResult
	{stackId}
	{projectId}
	result={combineResults(
		branchResult.current,
		branchesResult.current,
		branchDetailsResult.current,
		commitResult.current,
		upstreamOnlyCommits.current,
		localAndRemoteCommits.current
	)}
>
	{#snippet children(
		[branch, branches, branchDetails, commit, upstreamOnlyCommits, localAndRemoteCommits],
		{ projectId, stackId }
	)}
		{@const branchName = branch.name}
		{@const selected = selection.current?.branchName === branch.name}
		{@const isNewBranch = !upstreamOnlyCommits.length && !localAndRemoteCommits.length}
		{#if !first}
			<BranchDividerLine {commit} />
		{/if}
		<div class="branch-card" class:selected data-series-name={branchName}>
			<BranchHeader
				{projectId}
				{stackId}
				{branch}
				bind:el={headerEl}
				bind:menuBtnEl={kebabContextMenuTrigger}
				isMenuOpenByBtn={isContextMenuOpenedByBtn}
				isMenuOpenByMouse={isContextMenuOpenedByMouse}
				selected={selected && selection.current?.commitId === undefined}
				isTopBranch={first}
				{isNewBranch}
				readonly={!!branch.remoteTrackingBranch}
				onclick={() => {
					const stackState = uiState.stack(stackId);
					stackState.selection.set({ branchName });
					stackState.activeSelectionId.set({ type: 'branch', branchName, stackId });
					uiState.project(projectId).drawerPage.set('branch');
				}}
				onMenuBtnClick={() => {
					contextMenu?.toggle();
				}}
				onContextMenu={(e) => {
					e.preventDefault();
					e.stopPropagation();
					contextMenu?.toggle(e);
				}}
			>
				{#snippet details()}
					<div class="text-11 branch-header__details">
						<span class="branch-header__item">
							<BranchBadge pushStatus={branchDetails.pushStatus} unstyled />
						</span>
						<span class="branch-header__divider">•</span>

						{#if branchDetails.isConflicted}
							<span class="branch-header__item branch-header__item--conflict"> Has conflicts </span>
							<span class="branch-header__divider">•</span>
						{/if}

						<span class="branch-header__item">
							{getTimeAgo(new Date(branchDetails.lastUpdatedAt))}
						</span>

						{#if branch.reviewId || branch.prNumber}
							<span class="branch-header__divider">•</span>
							{#if branch.reviewId}
								<ReviewBadge brId={branch.reviewId} brStatus="unknown" />
							{/if}
							{#if branch.prNumber}
								<ReviewBadge prNumber={branch.prNumber} prStatus="unknown" />
							{/if}
						{/if}
					</div>
				{/snippet}
			</BranchHeader>

			{#if !isNewBranch}
				{@render commitList()}
			{/if}
		</div>

		<NewBranchModal
			{projectId}
			{stackId}
			bind:this={newBranchModal}
			parentSeriesName={branch.name}
		/>

		<SeriesHeaderContextMenu
			{projectId}
			bind:contextMenuEl={contextMenu}
			{stackId}
			leftClickTrigger={kebabContextMenuTrigger}
			rightClickTrigger={headerEl}
			branchName={branch.name}
			seriesCount={branches.length}
			isTopBranch={first}
			descriptionOption={false}
			onGenerateBranchName={() => {
				throw new Error('Not implemented!');
			}}
			onAddDependentSeries={() => newBranchModal?.show()}
			onOpenInBrowser={() => {
				const url = forgeBranch?.url;
				if (url) openExternalUrl(url);
			}}
			isPushed={!!branch.remoteTrackingBranch}
			branchType={commit?.state.type || 'LocalOnly'}
			onToggle={(isOpen, isLeftClick) => {
				if (isLeftClick) {
					isContextMenuOpenedByBtn = isOpen;
				} else {
					isContextMenuOpenedByMouse = isOpen;
				}
			}}
		/>
	{/snippet}
</ReduxResult>

<style>
	.branch-card {
		display: flex;
		flex-direction: column;
		width: 100%;
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
	}

	.branch-header__details {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px;
		color: var(--clr-text-2);
		margin-left: 4px;
	}

	.branch-header__item {
		white-space: nowrap;
		color: var(--clr-text-2);
	}

	.branch-header__item--conflict {
		color: var(--clr-theme-err-element);
	}

	.branch-header__divider {
		color: var(--clr-text-3);
	}
</style>
