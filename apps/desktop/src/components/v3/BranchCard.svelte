<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SeriesHeaderContextMenu from '$components/SeriesHeaderContextMenu.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchDividerLine from '$components/v3/BranchDividerLine.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import CommitGoesHere from '$components/v3/CommitGoesHere.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import NewBranchModal from '$components/v3/NewBranchModal.svelte';
	import { isLocalAndRemoteCommit, isUpstreamCommit } from '$components/v3/lib';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import {
		AmendCommitWithChangeDzHandler,
		type DzCommitData,
		SquashCommitDzHandler
	} from '$lib/commits/dropHandler';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';

	interface Props {
		projectId: string;
		stackId: string;
		branchName: string;
		first: boolean;
		last: boolean;
	}

	let { projectId, stackId, branchName, first, last: lastBranch }: Props = $props();

	const [stackService, baseBranchService, uiState, forge] = inject(
		StackService,
		BaseBranchService,
		UiState,
		DefaultForgeFactory
	);

	const branchResult = $derived(stackService.branchByName(projectId, stackId, branchName));
	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const branchDetailsResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const commitResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0));
	const localAndRemoteCommits = $derived(stackService.commits(projectId, stackId, branchName));
	const upstreamOnlyCommits = $derived(
		stackService.upstreamCommits(projectId, stackId, branchName)
	);
	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchResponse.current.data);
	const baseSha = $derived(base?.baseSha);

	const drawer = $derived(uiState.project(projectId).drawerPage.get());
	const isCommitting = $derived(drawer.current === 'new-commit');
	const selection = $derived(uiState.stack(stackId).selection.get());
	const selectedCommitId = $derived(selection.current?.commitId);
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
				readonly={!!forgeBranch}
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
				<BranchCommitList {projectId} {stackId} {branchName} {selectedCommitId}>
					{#snippet upstreamTemplate({ commit, first, lastCommit, selected })}
						{@const commitId = commit.id}
						{#if !isCommitting}
							<CommitRow
								{stackId}
								{branchName}
								{projectId}
								{first}
								lastCommit={lastCommit && !commit}
								{commit}
								{selected}
								onclick={() => {
									uiState.stack(stackId).selection.set({ branchName, commitId, upstream: true });
									uiState.project(projectId).drawerPage.set(undefined);
								}}
							/>
						{/if}
					{/snippet}
					{#snippet localAndRemoteTemplate({ commit, first, last, lastCommit, selected })}
						{@const commitId = commit.id}
						{#if isCommitting}
							<!-- Only commits to the base can be `last`, see next `CommitGoesHere`. -->
							<CommitGoesHere
								{selected}
								{first}
								last={false}
								onclick={() => uiState.stack(stackId).selection.set({ branchName, commitId })}
							/>
						{/if}
						{@const dzCommit: DzCommitData = {
						id: commit.id,
						isRemote: isUpstreamCommit(commit),
						isIntegrated: isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated',
						isConflicted: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
					}}
						{@const amendHandler = new AmendCommitWithChangeDzHandler(
							projectId,
							stackService,
							stackId,
							dzCommit,
							(newId) => uiState.stack(stackId).selection.set({ branchName, commitId: newId })
						)}
						{@const squashHandler = new SquashCommitDzHandler({
							stackService,
							projectId,
							stackId,
							commit: dzCommit
						})}
						<Dropzone handlers={[amendHandler, squashHandler]}>
							{#snippet overlay({ hovered, activated, handler })}
								{@const label =
									handler instanceof AmendCommitWithChangeDzHandler ? 'Amend' : 'Squash'}
								<CardOverlay {hovered} {activated} {label} />
							{/snippet}
							<CommitRow
								{stackId}
								{branchName}
								{projectId}
								{first}
								{lastCommit}
								{lastBranch}
								{commit}
								{selected}
								draggable
								onclick={() => {
									const stackState = uiState.stack(stackId);
									stackState.selection.set({ branchName, commitId });
									stackState.activeSelectionId.set({ type: 'commit', commitId });
									uiState.project(projectId).drawerPage.set(undefined);
								}}
							/>
						</Dropzone>
						{#if isCommitting && last && lastBranch}
							<CommitGoesHere
								{first}
								{last}
								selected={selectedCommitId === baseSha}
								onclick={() =>
									uiState.stack(stackId).selection.set({ branchName, commitId: baseSha })}
							/>
						{/if}
					{/snippet}
				</BranchCommitList>
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
			isTopBranch={true}
			descriptionOption={false}
			onGenerateBranchName={() => {
				throw new Error('Not implemented!');
			}}
			onAddDependentSeries={() => newBranchModal?.show()}
			onOpenInBrowser={() => {
				const url = forgeBranch?.url;
				if (url) openExternalUrl(url);
			}}
			hasForgeBranch={!!forgeBranch}
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
