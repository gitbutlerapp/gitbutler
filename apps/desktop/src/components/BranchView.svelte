<script lang="ts">
	import BranchDetails from '$components/BranchDetails.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import BranchRenameModal from '$components/BranchRenameModal.svelte';
	import BranchReview from '$components/BranchReview.svelte';
	import CommitRow from '$components/CommitRow.svelte';
	import DeleteBranchModal from '$components/DeleteBranchModal.svelte';
	import Drawer from '$components/Drawer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import newBranchSmolSVG from '$lib/assets/empty-state/new-branch-smol.svg?raw';
	import { commitCreatedAt, commitStateSubject } from '$lib/branches/v3';
	import { findEarliestConflict } from '$lib/commits/utils';
	import { editPatch } from '$lib/editMode/editPatchUtils';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Icon, TestId, Tooltip } from '@gitbutler/ui';

	import type { ComponentProps } from 'svelte';

	interface Props {
		stackId?: string;
		laneId: string;
		projectId: string;
		branchName: string;
		active?: boolean;
		grow?: boolean;
		clientHeight?: number;
		resizer?: Partial<ComponentProps<typeof Resizer>>;
		rounded?: boolean;
		ontoggle?: (collapsed: boolean) => void;
		onerror?: (err: unknown) => void;
		onclose?: () => void;
	}

	let {
		stackId,
		laneId,
		projectId,
		branchName,
		grow,
		clientHeight = $bindable(),
		resizer,
		rounded,
		ontoggle,
		onerror,
		onclose
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const modeService = inject(MODE_SERVICE);

	const branchQuery = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const branchesQuery = $derived(stackService.branches(projectId, stackId));
	const topCommitQuery = $derived(stackService.commitAt(projectId, stackId, branchName, 0));

	// Get conflicted commits for this branch
	const conflictedCommitsInBranch = $derived(
		branchQuery.response?.commits.filter((commit) => commit.hasConflicts) || []
	);

	let renameBranchModal = $state<BranchRenameModal>();
	let deleteBranchModal = $state<DeleteBranchModal>();

	// Handler for resolving conflicts - find the earliest conflicted commit
	async function handleResolveConflicts() {
		if (conflictedCommitsInBranch.length === 0 || !stackId) return;

		const ancestorMostConflicted = findEarliestConflict(conflictedCommitsInBranch);

		if (!ancestorMostConflicted) return;

		await editPatch({
			modeService,
			commitId: ancestorMostConflicted.id,
			stackId,
			projectId
		});
	}
</script>

<ReduxResult
	{stackId}
	{projectId}
	{onerror}
	result={combineResults(branchesQuery.result, branchQuery.result, topCommitQuery.result)}
>
	{#snippet children([branches, branch, topCommit], { stackId, projectId })}
		{@const hasCommits = !!topCommit || branch.upstreamCommits.length > 0}
		{@const remoteTrackingBranch = branch.remoteTrackingBranch}
		<Drawer
			bind:clientHeight
			persistId="branch-view-drawer-{projectId}-{stackId}-{branch.name}"
			testId={TestId.BranchView}
			{resizer}
			{grow}
			{onclose}
			{ontoggle}
			{rounded}
			noshrink
		>
			{#snippet header()}
				<div class="branch__header">
					{#if hasCommits}
						<Tooltip
							text={remoteTrackingBranch
								? `Remote tracking branch:\n${remoteTrackingBranch}`
								: 'No remote tracking branch'}
						>
							<div class="remote-tracking-branch-icon" class:disabled={!remoteTrackingBranch}>
								<Icon
									name={remoteTrackingBranch ? 'remote-target-branch' : 'no-remote-target-branch'}
								/>
							</div>
						</Tooltip>
					{/if}
					<h3 class="text-15 text-bold truncate">{branch.name}</h3>
				</div>
			{/snippet}

			{#snippet actions(header)}
				{@const data = {
					branch,
					prNumber: branch.prNumber || undefined,
					stackLength: branches.length
				}}
				<BranchHeaderContextMenu
					{projectId}
					{stackId}
					{laneId}
					rightClickTrigger={header}
					contextData={data}
				/>
			{/snippet}

			{#if hasCommits}
				<div class="branch-view">
					<BranchDetails {branch} onResolveConflicts={handleResolveConflicts}>
						<BranchReview
							{stackId}
							{projectId}
							branchName={branch.name}
							prNumber={branch.prNumber || undefined}
							reviewId={branch.reviewId || undefined}
						/>

						{#snippet conflictedCommits()}
							{#if conflictedCommitsInBranch.length > 0}
								{#each conflictedCommitsInBranch as commit}
									{@const isLocalAndRemote = commit.state.type === 'LocalAndRemote'}
									<CommitRow
										type={commit.state.type}
										{branchName}
										commitId={commit.id}
										commitMessage={commit.message}
										gerritReviewUrl={commit.gerritReviewUrl ?? undefined}
										createdAt={commitCreatedAt(commit)}
										hasConflicts={true}
										disableCommitActions={true}
										diverged={isLocalAndRemote && commit.id !== commitStateSubject(commit)}
										active
										onclick={() => {
											// Open commit preview by setting selection
											const laneState = uiState.lane(laneId);
											laneState.selection.set({
												branchName,
												commitId: commit.id,
												previewOpen: true
											});
										}}
									/>
								{/each}
							{/if}
						{/snippet}
					</BranchDetails>
				</div>
			{:else}
				<div class="branch-view__empty-state">
					<div class="branch-view__empty-state__image">
						{@html newBranchSmolSVG}
					</div>
					<h3 class="text-16 text-semibold branch-view__empty-state__title">
						This is a new branch
					</h3>
					<p class="text-13 text-body branch-view__empty-state__description">
						Commit your changes here. You can stack additional branches or apply them independently.
						You can also drag and drop files to start a new commit.
					</p>
				</div>
			{/if}
		</Drawer>

		<BranchRenameModal
			{projectId}
			{stackId}
			{laneId}
			branchName={branch.name}
			bind:this={renameBranchModal}
			isPushed={!!branch.remoteTrackingBranch}
		/>
		<DeleteBranchModal
			{projectId}
			{stackId}
			branchName={branch.name}
			bind:this={deleteBranchModal}
		/>
	{/snippet}
</ReduxResult>

<style>
	.branch__header {
		display: flex;
		align-items: center;
		width: 100%;
		overflow: hidden;
		gap: 8px;
	}

	.remote-tracking-branch-icon {
		display: flex;
		gap: 6px;
		color: var(--clr-text-1);
		opacity: 0.5;
		transition: var(--transition-fast);

		&:hover {
			opacity: 0.7;
		}

		&.disabled {
			opacity: 0.3;
		}
	}

	.branch-view {
		position: relative;
		/* Limit the commit view to at most 40vh to ensure other sections remain visible */
		max-height: 50vh;
	}

	.branch-view__empty-state {
		display: flex;
		flex: 1;
		flex-direction: column;
		justify-content: center;
		max-width: 540px;
		padding: 30px;
	}

	.branch-view__empty-state__image {
		width: 180px;
		margin-bottom: 20px;
	}

	.branch-view__empty-state__title {
		margin-bottom: 10px;
	}

	.branch-view__empty-state__description {
		color: var(--clr-text-2);
		text-wrap: balance;
	}
</style>
