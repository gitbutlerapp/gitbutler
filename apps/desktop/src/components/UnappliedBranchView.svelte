<script lang="ts">
	import BranchDetails from '$components/BranchDetails.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import BranchReview from '$components/BranchReview.svelte';
	import ChangedFiles from '$components/ChangedFiles.svelte';
	import Drawer from '$components/Drawer.svelte';
	import KebabButton from '$components/KebabButton.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import { Icon, Tooltip } from '@gitbutler/ui';

	import type { BranchHeaderContextItem } from '$components/BranchHeaderContextMenu.svelte';
	import type { SelectionId } from '$lib/selection/key';

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

	const branchResult = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName, remote)
	);
	const changesResult = $derived(stackService.branchChanges({ projectId, branchName, remote }));
	let headerMenuContext = $state<BranchHeaderContextItem>();

	const selectionId: SelectionId = $derived.by(() => {
		const bname = remote ? remote + '/' + branchName : branchName;
		if (stackId) {
			return {
				type: 'branch',
				branchName: bname,
				stackId
			};
		}
		return {
			type: 'branch',
			branchName: bname
		};
	});
</script>

<ReduxResult {projectId} result={branchResult.current} {onerror}>
	{#snippet children(branch, { stackId, projectId })}
		{@const hasCommits = branch.commits.length > 0}
		{@const remoteTrackingBranch = branch.remoteTrackingBranch}
		{@const preferredPrNumber = branch.prNumber || prNumber}
		<Drawer testId={TestId.UnappliedBranchDrawer} {onclose} bottomBorder>
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
					stackLength: 1
				}}
				<KebabButton
					contextElement={header}
					onclick={(element) => (headerMenuContext = { data, position: { element } })}
					oncontext={(coords) => (headerMenuContext = { data, position: { coords } })}
					activated={!!headerMenuContext?.position.element}
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

		<ReduxResult {projectId} result={changesResult.current}>
			{#snippet children(changes, env)}
				<ChangedFiles
					title="All changed files"
					projectId={env.projectId}
					stackId={env.stackId}
					active
					autoselect
					grow
					{selectionId}
					{changes}
				/>
			{/snippet}
		</ReduxResult>
	{/snippet}
</ReduxResult>

{#if headerMenuContext}
	<BranchHeaderContextMenu {projectId} bind:context={headerMenuContext} />
{/if}

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
