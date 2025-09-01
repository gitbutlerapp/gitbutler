<script lang="ts">
	import BranchDetails from '$components/BranchDetails.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import BranchRenameModal from '$components/BranchRenameModal.svelte';
	import BranchReview from '$components/BranchReview.svelte';
	import DeleteBranchModal from '$components/DeleteBranchModal.svelte';
	import Drawer from '$components/Drawer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import newBranchSmolSVG from '$lib/assets/empty-state/new-branch-smol.svg?raw';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
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
		ontoggle,
		onerror,
		onclose
	}: Props = $props();

	const stackService = inject(STACK_SERVICE);

	const branchResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const branchesResult = $derived(stackService.branches(projectId, stackId));
	const topCommitResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0));

	let renameBranchModal = $state<BranchRenameModal>();
	let deleteBranchModal = $state<DeleteBranchModal>();
</script>

<ReduxResult
	{stackId}
	{projectId}
	{onerror}
	result={combineResults(branchesResult.current, branchResult.current, topCommitResult.current)}
>
	{#snippet children([branches, branch, topCommit], { stackId, projectId })}
		{@const hasCommits = !!topCommit || branch.upstreamCommits.length > 0}
		{@const remoteTrackingBranch = branch.remoteTrackingBranch}
		<Drawer
			bind:clientHeight
			testId={TestId.BranchView}
			{resizer}
			{grow}
			{onclose}
			{ontoggle}
			bottomBorder
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

			{#snippet kebabMenu(header)}
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
				<BranchDetails {branch}>
					<BranchReview
						{stackId}
						{projectId}
						branchName={branch.name}
						prNumber={branch.prNumber || undefined}
						reviewId={branch.reviewId || undefined}
					/>
				</BranchDetails>
			{:else}
				<div class="branch-view__empty-state">
					<div class="branch-view__empty-state__image">
						{@html newBranchSmolSVG}
					</div>
					<h3 class="text-18 text-semibold branch-view__empty-state__title">
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
