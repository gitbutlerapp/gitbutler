<script lang="ts">
	import BranchRenameModal from '$components/BranchRenameModal.svelte';
	import BranchReview from '$components/BranchReview.svelte';
	import DeleteBranchModal from '$components/DeleteBranchModal.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SeriesHeaderContextMenuContents from '$components/SeriesHeaderContextMenuContents.svelte';
	import BranchDetails from '$components/v3/BranchDetails.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import NewBranchModal from '$components/v3/NewBranchModal.svelte';
	import newBranchSmolSVG from '$lib/assets/empty-state/new-branch-smol.svg?raw';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { TestId } from '$lib/testing/testIds';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	interface Props {
		stackId: string;
		projectId: string;
		branchName: string;
	}

	const { stackId, projectId, branchName }: Props = $props();

	const [stackService, forge] = inject(StackService, DefaultForgeFactory);

	const branchesResult = $derived(stackService.branches(projectId, stackId));

	const branchResult = $derived(stackService.branchDetails(projectId, stackId, branchName));
	const topCommitResult = $derived(stackService.commitAt(projectId, stackId, branchName, 0));

	const changesResult = stackService.branchChanges(projectId, stackId, branchName);
	const forgeBranch = $derived(forge.current?.branch(branchName));

	// context menu
	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let kebabTrigger = $state<HTMLButtonElement>();
	let isContextMenuOpen = $state(false);

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();
	let renameBranchModal = $state<BranchRenameModal>();
	let deleteBranchModal = $state<DeleteBranchModal>();
</script>

<ReduxResult
	{stackId}
	{projectId}
	result={combineResults(branchesResult.current, branchResult.current, topCommitResult.current)}
>
	{#snippet children([branches, branch, topCommit], { stackId, projectId })}
		{@const hasCommits = !!topCommit}
		{@const remoteTrackingBranch = branch.remoteTrackingBranch}
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

			{#snippet filesSplitView()}
				<ReduxResult {projectId} {stackId} result={changesResult.current}>
					{#snippet children(changes, { projectId, stackId })}
						<ChangedFiles
							title="All changed files"
							{projectId}
							{stackId}
							selectionId={{ type: 'branch', stackId, branchName }}
							{changes}
						/>
					{/snippet}
				</ReduxResult>
			{/snippet}
		</Drawer>

		<NewBranchModal {projectId} {stackId} bind:this={newBranchModal} />

		<ContextMenu
			bind:this={contextMenu}
			testId={TestId.BranchHeaderContextMenu}
			leftClickTrigger={kebabTrigger}
		>
			<SeriesHeaderContextMenuContents
				{projectId}
				contextMenuEl={contextMenu}
				{stackId}
				branchName={branch.name}
				seriesCount={branches.length}
				isTopBranch={branches[0]?.name === branch.name}
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
				branchType={topCommit?.state.type || 'LocalOnly'}
				showBranchRenameModal={() => {
					renameBranchModal?.show();
				}}
				showDeleteBranchModal={() => {
					deleteBranchModal?.show();
				}}
			/>
		</ContextMenu>
		<BranchRenameModal
			{projectId}
			{stackId}
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

	/* EMPTY STATE */
	.branch-view__empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
		padding: 30px;
		max-width: 540px;
		margin: 0 auto;
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
