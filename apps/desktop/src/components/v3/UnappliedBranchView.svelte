<script lang="ts">
	import BranchReview from '$components/BranchReview.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchDetails from '$components/v3/BranchDetails.svelte';
	import BranchHeaderContextMenu from '$components/v3/BranchHeaderContextMenu.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import KebabButton from '$components/v3/KebabButton.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import type { BranchHeaderContextItem } from '$components/v3/BranchHeaderContextMenu.svelte';

	interface Props {
		projectId: string;
		branchName: string;
		stackId?: string;
		remote?: string;
		prNumber?: number;
	}

	const { projectId, stackId, branchName, remote, prNumber }: Props = $props();

	const [stackService] = inject(StackService, BaseBranchService);

	const branchResult = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName, remote)
	);
	const changesResult = $derived(stackService.branchChanges({ projectId, branchName, remote }));
	let headerMenuContext = $state<BranchHeaderContextItem>();
</script>

<ReduxResult {projectId} result={branchResult.current}>
	{#snippet children(branch, { stackId, projectId })}
		{@const hasCommits = branch.commits.length > 0}
		{@const remoteTrackingBranch = branch.remoteTrackingBranch}
		{@const preferredPrNumber = branch.prNumber || prNumber}
		<Drawer {projectId} {stackId}>
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
					prNumber: branch.prNumber || undefined
				}}
				<KebabButton
					flat
					contextElement={header}
					onclick={(element) => (headerMenuContext = { data, position: { element } })}
					oncontext={(coords) => (headerMenuContext = { data, position: { coords } })}
					open={branchName === headerMenuContext?.data.branch.name}
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

			{#snippet filesSplitView()}
				<ReduxResult {projectId} result={changesResult.current}>
					{#snippet children(changes, env)}
						<ChangedFiles
							title="All changed files"
							projectId={env.projectId}
							stackId={env.stackId}
							selectionId={remote
								? { type: 'branch', branchName: remote + '/' + branchName }
								: { type: 'branch', branchName }}
							{changes}
						/>
					{/snippet}
				</ReduxResult>
			{/snippet}
		</Drawer>
	{/snippet}
</ReduxResult>

{#if headerMenuContext}
	<BranchHeaderContextMenu {projectId} bind:context={headerMenuContext} />
{/if}

<style>
	.branch__header {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		overflow: hidden;
	}

	.metadata {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		overflow: hidden;
	}

	/*  */
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
