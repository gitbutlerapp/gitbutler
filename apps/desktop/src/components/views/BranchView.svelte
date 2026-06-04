<script lang="ts">
	import BranchDetails from "$components/branch/BranchDetails.svelte";
	import BranchHeaderContextMenu from "$components/branch/BranchHeaderContextMenu.svelte";
	import BranchRenameModal from "$components/branch/BranchRenameModal.svelte";
	import DeleteBranchModal from "$components/branch/DeleteBranchModal.svelte";
	import CommitListItem from "$components/commit/CommitListItem.svelte";
	import BranchReview from "$components/forge/BranchReview.svelte";
	import Drawer from "$components/shared/Drawer.svelte";
	import newBranchSmolSVG from "$lib/assets/empty-state/new-branch-smol.svg?raw";
	import { commitCreatedAt, commitStateSubject } from "$lib/branches/v3";
	import { findEarliestConflict } from "$lib/commits/utils";
	import { editPatch } from "$lib/mode/editPatchUtils";
	import { MODE_SERVICE } from "$lib/mode/modeService";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Icon, TestId, Tooltip, Button } from "@gitbutler/ui";
	import type { Segment } from "@gitbutler/but-sdk";

	interface Props {
		stackId?: string;
		laneId: string;
		projectId: string;
		branchName: string;
		segment: Segment;
		branchIndex: number;
		parent: Segment | undefined;
		child: Segment | undefined;
		withForce: boolean;
		stackPrNumbers: (number | undefined)[];
		stackLength: number;
		active?: boolean;
		grow?: boolean;
		clientHeight?: number;
		rounded?: boolean;
		ontoggle?: (collapsed: boolean) => void;
		onclose?: () => void;
		onpopout?: () => void;
	}

	let {
		stackId,
		laneId,
		projectId,
		branchName,
		segment,
		branchIndex,
		parent,
		child,
		withForce,
		stackPrNumbers,
		stackLength,
		grow,
		clientHeight = $bindable(),
		rounded,
		ontoggle,
		onclose,
		onpopout,
	}: Props = $props();

	const uiState = inject(UI_STATE);
	const modeService = inject(MODE_SERVICE);

	const topCommit = $derived(segment.commits.at(0));
	const hasCommits = $derived(!!topCommit || segment.commitsOnRemote.length > 0);
	const remoteTrackingBranch = $derived(
		segment.remoteTrackingRefName
			? new TextDecoder().decode(new Uint8Array(segment.remoteTrackingRefName.fullNameBytes))
			: undefined,
	);
	const prNumber = $derived(segment.metadata?.review.pullRequest ?? undefined);
	const reviewId = $derived(segment.metadata?.review.reviewId ?? undefined);
	const authors = $derived(
		Array.from(
			new Map(
				[...segment.commits, ...segment.commitsOnRemote].map((commit) => [
					JSON.stringify(commit.author),
					commit.author,
				]),
			).values(),
		),
	);
	const isConflicted = $derived(segment.commits.some((commit) => commit.hasConflicts));

	// Get conflicted commits for this branch
	const conflictedCommitsInBranch = $derived(
		segment.commits.filter((commit) => commit.hasConflicts),
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
			projectId,
		});
	}
</script>

<Drawer
	bind:clientHeight
	persistId="branch-view-drawer-{projectId}-{stackId}-{branchName}"
	testId={TestId.BranchView}
	{grow}
	{onclose}
	{ontoggle}
	{rounded}
	noshrink
>
	{#snippet closeActions()}
		{#if onpopout}
			<Button
				kind="ghost"
				icon="pop-out-bottom-right"
				size="tag"
				tooltip="Pop out diff view"
				onclick={onpopout}
			/>
		{/if}
	{/snippet}
	{#snippet header()}
		<div class="branch__header">
			{#if hasCommits}
				<Tooltip
					text={remoteTrackingBranch
						? `Remote tracking branch:\n${remoteTrackingBranch}`
						: "No remote tracking branch"}
				>
					<div class="remote-tracking-branch-icon" class:disabled={!remoteTrackingBranch}>
						<Icon name={remoteTrackingBranch ? "target-branch" : "target-cross"} />
					</div>
				</Tooltip>
			{/if}
			<h3 class="text-15 text-bold truncate">{branchName}</h3>
		</div>
	{/snippet}

	{#snippet actions()}
		{@const data = {
			segment,
			prNumber,
			stackLength,
		}}
		<BranchHeaderContextMenu {projectId} {stackId} {laneId} contextData={data} />
	{/snippet}

	{#if hasCommits}
		<div class="branch-view">
			<BranchDetails
				pushStatus={segment.pushStatus}
				{authors}
				{isConflicted}
				onResolveConflicts={handleResolveConflicts}
			>
				<BranchReview
					{stackId}
					{projectId}
					{branchName}
					{segment}
					{branchIndex}
					{parent}
					{child}
					{withForce}
					{stackPrNumbers}
					{prNumber}
					{reviewId}
				/>

				{#snippet conflictedCommits()}
					{#if conflictedCommitsInBranch.length > 0}
						{#each conflictedCommitsInBranch as commit}
							{@const isLocalAndRemote = commit.state.type === "LocalAndRemote"}
							<CommitListItem
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
										previewOpen: true,
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
			<h3 class="text-16 text-semibold branch-view__empty-state__title">This is a new branch</h3>
			<p class="text-13 text-body branch-view__empty-state__description">
				Commit your changes here. You can stack additional branches or apply them independently. You
				can also drag and drop files to start a new commit.
			</p>
		</div>
	{/if}
</Drawer>

<BranchRenameModal
	{projectId}
	{stackId}
	{laneId}
	{branchName}
	bind:this={renameBranchModal}
	isPushed={!!remoteTrackingBranch}
/>
<DeleteBranchModal {projectId} {stackId} {branchName} bind:this={deleteBranchModal} />

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
		color: var(--text-1);
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
		color: var(--text-2);
		text-wrap: balance;
	}
</style>
