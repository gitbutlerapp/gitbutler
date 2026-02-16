<script lang="ts">
	import BranchDetails from '$components/BranchDetails.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import BranchReview from '$components/BranchReview.svelte';
	import Drawer from '$components/Drawer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
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
</script>

<ReduxResult {projectId} result={branchQuery.result} {onerror}>
	{#snippet children(branch, { stackId, projectId })}
		{@const hasCommits = branch.commits.length > 0}
		{@const remoteTrackingBranch = branch.remoteTrackingBranch}
		{@const preferredPrNumber = branch.prNumber || prNumber}
		<Drawer testId={TestId.UnappliedBranchDrawer} {onclose} noshrink rounded>
			{#snippet header()}
				{#if hasCommits}
					<Tooltip
						text={remoteTrackingBranch
							? `Remote tracking branch:\n${remoteTrackingBranch}`
							: 'No remote tracking branch'}
					>
						<Icon
							name={remoteTrackingBranch ? 'remote-target-branch' : 'no-remote-target-branch'}
						/>
					</Tooltip>
				{/if}
				<span class="text-15 text-bold truncate">{branch.name}</span>
			{/snippet}

			{#snippet actions(header)}
				<BranchHeaderContextMenu
					laneId="unapplied-branch-view"
					{projectId}
					rightClickTrigger={header}
					contextData={{
						branch,
						prNumber: branch.prNumber || undefined,
						stackLength: 1
					}}
				/>
			{/snippet}

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
		</Drawer>
	{/snippet}
</ReduxResult>

<style>
	.metadata {
		display: flex;
		align-items: center;
		width: 100%;
		overflow: hidden;
		gap: 8px;
	}
</style>
