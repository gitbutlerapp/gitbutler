<script lang="ts">
	import BranchDetails from '$components/BranchDetails.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import BranchReview from '$components/BranchReview.svelte';
	import ChangedFiles from '$components/ChangedFiles.svelte';
	import Drawer from '$components/Drawer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { createBranchSelection, type SelectionId } from '$lib/selection/key';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { createBranchRef } from '$lib/utils/branch';
	import { inject } from '@gitbutler/core/context';
	import { Icon, TestId, Tooltip } from '@gitbutler/ui';

	interface Props {
		projectId: string;
		branchName: string;
		stackId?: string;
		remote?: string;
		prNumber?: number;
		onerror?: (err: unknown) => void;
		onclose?: () => void;
	}

	const { projectId, stackId, branchName, remote, prNumber, onerror, onclose }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	const branchQuery = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName, remote)
	);

	const selectionId: SelectionId = $derived.by(() => {
		return createBranchSelection({ stackId, branchName, remote });
	});

	const branchRef = $derived(createBranchRef(branchName, remote));
</script>

<ReduxResult {projectId} result={branchQuery.result} {onerror}>
	{#snippet children(branch, { stackId, projectId })}
		{@const hasCommits = branch.commits.length > 0}
		{@const remoteTrackingBranch = branch.remoteTrackingBranch}
		{@const preferredPrNumber = branch.prNumber || prNumber}
		<Drawer
			testId={TestId.UnappliedBranchDrawer}
			persistId="unapplied-branch-drawer-{projectId}-{branch.name}"
			{onclose}
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
					stackLength: 1
				}}
				<BranchHeaderContextMenu
					laneId="unapplied-branch-view"
					{projectId}
					rightClickTrigger={header}
					contextData={data}
				/>
			{/snippet}

			<div class="branch-header">
				<div class="metadata">
					<BranchDetails {branch}>
						{#if preferredPrNumber}
							<!-- TODO(mattias): Use pr number from branch. -->
							<BranchReview
								{stackId}
								{projectId}
								{prNumber}
								branchName={branch.name}
								reviewId={branch.reviewId || undefined}
							/>
						{/if}
					</BranchDetails>
				</div>
			</div>
		</Drawer>

		{@const changesQuery = stackService.branchChanges({ projectId, branch: branchRef })}
		<ReduxResult {projectId} result={changesQuery.result}>
			{#snippet children(changes, env)}
				<ChangedFiles
					title="All changed files"
					projectId={env.projectId}
					stackId={env.stackId}
					autoselect
					grow
					{selectionId}
					persistId={`unapplied-branch-${branchName}`}
					changes={changes.changes}
					stats={changes.stats}
					allowUnselect={false}
				/>
			{/snippet}
		</ReduxResult>
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

	.metadata {
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
</style>
