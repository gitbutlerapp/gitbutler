<script lang="ts">
	import BranchReview from '$components/BranchReview.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchDetails from '$components/v3/BranchDetails.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	interface Props {
		projectId: string;
		branchName: string;
		stackId?: string;
		remote?: string;
		prNumber?: number;
		hasLocal?: boolean;
		isTarget?: boolean;
	}

	const { projectId, stackId, branchName, remote, prNumber, hasLocal, isTarget }: Props = $props();

	const [stackService, baseBranchService] = inject(StackService, BaseBranchService);

	const branchResult = $derived(
		stackId
			? stackService.branchDetails(projectId, stackId, branchName)
			: stackService.unstackedBranchDetails(projectId, branchName, remote)
	);
	const changesResult = $derived(stackService.branchChanges({ projectId, branchName, remote }));

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabTrigger = $state<HTMLButtonElement>();
	let isContextMenuOpen = $state(false);

	async function checkoutBranch() {
		const remoteRef = remote ? `refs/remotes/${remote}/${branchName}` : undefined;
		const branchRef = hasLocal ? `refs/heads/${branchName}` : remoteRef;
		if (branchRef) {
			await stackService.createVirtualBranchFromBranch({
				projectId,
				branch: branchRef,
				remote: remoteRef,
				prNumber
			});
			await baseBranchService.refreshBaseBranch(projectId);
		}
	}

	async function deleteLocalBranch() {
		await stackService.deleteLocalBranch({
			projectId,
			refname: `refs/heads/${branchName}`,
			givenName: branchName
		});
		await baseBranchService.refreshBaseBranch(projectId);
	}
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

			{#snippet kebabMenu()}
				<Button
					size="tag"
					icon="kebab"
					kind="ghost"
					activated={isContextMenuOpen}
					bind:el={kebabTrigger}
					onclick={() => {
						contextMenu?.toggle();
					}}
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

				<div class="actions">
					{#if !isTarget}
						<AsyncButton
							size="tag"
							kind="outline"
							icon="edit-small"
							action={async () => {
								await checkoutBranch();
							}}
						>
							Apply
						</AsyncButton>
					{/if}
					{#if hasLocal}
						<AsyncButton
							size="tag"
							kind="outline"
							icon="edit-small"
							action={async () => {
								await deleteLocalBranch();
							}}
						>
							Delete local
						</AsyncButton>
					{/if}
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

	.actions {
		width: 100%;
		display: flex;
		gap: 5px;
		margin-top: 14px;
	}
</style>
