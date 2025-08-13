<script lang="ts">
	import AddDependentBranchModal, {
		type AddDependentBranchModalProps
	} from '$components/AddDependentBranchModal.svelte';
	import BranchCard from '$components/BranchCard.svelte';
	import BranchCommitList from '$components/BranchCommitList.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import ConflictResolutionConfirmModal from '$components/ConflictResolutionConfirmModal.svelte';
	import NewBranchModal from '$components/NewBranchModal.svelte';
	import PushButton from '$components/PushButton.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { getColorFromCommitState, getIconFromCommitState } from '$components/lib';
	import { STACKING_REORDER_DROPZONE_MANAGER_FACTORY } from '$lib/dragging/stackingReorderDropzoneManager';
	import { editPatch } from '$lib/editMode/editPatchUtils';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { INTELLIGENT_SCROLLING_SERVICE } from '$lib/intelligentScrolling/service';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { inject } from '@gitbutler/shared/context';

	import { Button, Modal, TestId } from '@gitbutler/ui';
	import { getForgeLogo } from '@gitbutler/ui/utils/getForgeLogo';
	import { QueryStatus } from '@reduxjs/toolkit/query';
	import { tick } from 'svelte';
	import type { CommitStatusType } from '$lib/commits/commit';
	import type { BranchDetails } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stackId?: string;
		laneId: string;
		branches: BranchDetails[];
		focusedStackId?: string;
		onselect?: () => void;
	};

	const { projectId, branches, stackId, laneId, focusedStackId, onselect }: Props = $props();
	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const modeService = inject(MODE_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const intelligentScrollingService = inject(INTELLIGENT_SCROLLING_SERVICE);

	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;

	let addDependentBranchModalContext = $state<AddDependentBranchModalProps>();
	let addDependentBranchModal = $state<AddDependentBranchModal>();

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(
		exclusiveAction?.type === 'commit' && exclusiveAction?.stackId === stackId
	);
	const laneState = $derived(uiState.lane(laneId));
	const selection = $derived(laneState.selection);
	const selectedCommitId = $derived(selection.current?.commitId);

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();

	let conflictResolutionConfirmationModal =
		$state<ReturnType<typeof ConflictResolutionConfirmModal>>();

	async function handleUncommit(commitId: string, branchName: string) {
		if (!stackId) return;
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commitId });
	}

	function startEditingCommitMessage(branchName: string, commitId: string) {
		laneState.selection.set({ branchName, commitId });
		projectState.exclusiveAction.set({
			type: 'edit-commit-message',
			stackId,
			branchName,
			commitId
		});
	}

	async function handleEditPatch(args: {
		commitId: string;
		type: CommitStatusType;
		hasConflicts: boolean;
		isAncestorMostConflicted: boolean;
	}) {
		if (!stackId) return;
		if (args.type === 'LocalAndRemote' && args.hasConflicts && !args.isAncestorMostConflicted) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		await editPatch({
			modeService,
			commitId: args.commitId,
			stackId,
			projectId
		});
	}

	const selectedCommit = $derived(
		selectedCommitId ? stackService.commitDetails(projectId, selectedCommitId) : undefined
	);

	$effect(() => {
		if (selectedCommit && selectedCommit.current.status === QueryStatus.rejected) {
			const branchName = selection.current?.branchName;
			if (branchName) {
				selection.set({ branchName, commitId: undefined });
			}
		}
	});

	const stackingReorderDropzoneManagerFactory = inject(STACKING_REORDER_DROPZONE_MANAGER_FACTORY);
	const stackingReorderDropzoneManager = $derived(
		stackingReorderDropzoneManagerFactory.build(
			laneId,
			branches.map((s) => ({ name: s.name, commitIds: s.commits.map((p) => p.id) }))
		)
	);

	const canPublishPR = $derived(forge.current.authenticated);
</script>

<div class="branches-wrapper">
	{#each branches as branch, i}
		{@const branchName = branch.name}
		{@const localAndRemoteCommits = stackService.commits(projectId, stackId, branchName)}
		{@const upstreamOnlyCommits = stackService.upstreamCommits(projectId, stackId, branchName)}
		{@const branchDetailsResult = stackService.branchDetails(projectId, stackId, branchName)}
		{@const commitResult = stackService.commitAt(projectId, stackId, branchName, 0)}
		{@const prResult = branch.prNumber ? forge.current.prService?.get(branch.prNumber) : undefined}

		{@const first = i === 0}

		<ReduxResult
			{projectId}
			{stackId}
			result={combineResults(
				localAndRemoteCommits.current,
				upstreamOnlyCommits.current,
				branchDetailsResult.current,
				commitResult.current
			)}
		>
			{#snippet children([localAndRemoteCommits, upstreamOnlyCommits, branchDetails, commit])}
				{@const firstBranch = i === 0}
				{@const lastBranch = i === branches.length - 1}
				{@const iconName = getIconFromCommitState(commit?.id, commit?.state)}
				{@const lineColor = commit
					? getColorFromCommitState(
							commit.state.type,
							commit.state.type === 'LocalAndRemote' && commit.id !== commit.state.subject
						)
					: 'var(--clr-commit-local)'}
				{@const isNewBranch =
					upstreamOnlyCommits.length === 0 && localAndRemoteCommits.length === 0}
				{@const selected =
					selection?.current?.branchName === branchName &&
					selection?.current.commitId === undefined}
				{@const pushStatus = branchDetails.pushStatus}
				{@const isConflicted = branchDetails.isConflicted}
				{@const lastUpdatedAt = branchDetails.lastUpdatedAt}
				{@const reviewId = branch.reviewId || undefined}
				{@const prNumber = branch.prNumber || undefined}
				<BranchCard
					type="stack-branch"
					{projectId}
					{stackId}
					{laneId}
					{branchName}
					{lineColor}
					{first}
					{isCommitting}
					{iconName}
					{selected}
					{isNewBranch}
					{pushStatus}
					{isConflicted}
					{lastUpdatedAt}
					{reviewId}
					{prNumber}
					dropzones={[stackingReorderDropzoneManager.top(branchName)]}
					active={focusedStackId === stackId}
					trackingBranch={branch.remoteTrackingBranch ?? undefined}
					readonly={!!branch.remoteTrackingBranch}
					onclick={() => {
						uiState.lane(laneId).selection.set({ branchName });
						intelligentScrollingService.show(projectId, laneId, 'details');
						onselect?.();
					}}
				>
					{#snippet buttons()}
						<Button
							icon="new-empty-commit"
							size="tag"
							kind="outline"
							tooltip="Create empty commit"
							onclick={async () => {
								if (!stackId) return;
								await insertBlankCommitInBranch({
									projectId,
									stackId,
									commitId: undefined,
									offset: -1
								});
							}}
							disabled={commitInsertion.current.isLoading}
						/>
						<Button
							icon="copy-small"
							size="tag"
							kind="outline"
							tooltip="Copy branch name"
							onclick={() => {
								copyToClipboard(branchName);
							}}
						/>
						{#if first}
							<Button
								icon="new-dep-branch"
								size="tag"
								kind="outline"
								tooltip="Create new branch"
								onclick={async () => {
									if (!stackId) return;
									addDependentBranchModalContext = {
										projectId,
										stackId
									};

									await tick();
									addDependentBranchModal?.show();
								}}
							/>
						{/if}

						{#if canPublishPR && !isNewBranch}
							{#if !branch.prNumber}
								<Button
									size="tag"
									kind="outline"
									shrinkable
									onclick={(e) => {
										e.stopPropagation();
										projectState.exclusiveAction.set({
											type: 'create-pr',
											stackId,
											branchName
										});
									}}
									testId={TestId.CreateReviewButton}
									disabled={!!projectState.exclusiveAction.current}
									icon={getForgeLogo(forge.current.name, true)}
								>
									{`Create ${forge.current.name === 'gitlab' ? 'MR' : 'PR'}`}
								</Button>
							{:else}
								{@const prUrl = prResult?.current.data?.htmlUrl}
								<Button
									size="tag"
									kind="outline"
									shrinkable
									disabled={!prUrl}
									onclick={() => {
										if (prUrl) {
											openExternalUrl(prUrl);
										}
									}}
									icon={forge.current.name === 'gitlab' ? 'view-mr-browser' : 'view-pr-browser'}
								>
									{`View ${forge.current.name === 'gitlab' ? 'MR' : 'PR'}`}
								</Button>
							{/if}
						{/if}
						<PushButton
							{branchName}
							{projectId}
							{stackId}
							multipleBranches={branches.length > 1}
							isFirstBranchInStack={firstBranch}
							isLastBranchInStack={lastBranch}
						/>
					{/snippet}

					{#snippet menu({ rightClickTrigger })}
						{@const data = {
							branch,
							prNumber,
							first,
							stackLength: branches.length
						}}
						<BranchHeaderContextMenu
							{projectId}
							{stackId}
							{laneId}
							{rightClickTrigger}
							contextData={data}
						/>
					{/snippet}

					{#snippet branchContent()}
						<BranchCommitList
							{firstBranch}
							{lastBranch}
							active={focusedStackId === stackId}
							{projectId}
							{stackId}
							{laneId}
							{branchName}
							{branchDetails}
							{stackingReorderDropzoneManager}
							{handleUncommit}
							{startEditingCommitMessage}
							{handleEditPatch}
							{onselect}
						/>
					{/snippet}
				</BranchCard>
			{/snippet}
		</ReduxResult>
	{/each}
</div>

<NewBranchModal {projectId} {stackId} bind:this={newBranchModal} />

<Modal
	bind:this={conflictResolutionConfirmationModal}
	width="small"
	defaultItem={{} as {
		type: CommitStatusType;
		commitId: string;
		stackId: string;
		hasConflicts: boolean;
		isAncestorMostConflicted: boolean;
		close: () => void;
	}}
	onSubmit={async (close, item) => {
		await handleEditPatch(item);
		close();
	}}
>
	<div>
		<p>It's generally better to start resolving conflicts from the bottom up.</p>
		<br />
		<p>Are you sure you want to resolve conflicts for this commit?</p>
	</div>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit">Yes</Button>
	{/snippet}
</Modal>

{#if addDependentBranchModalContext}
	<AddDependentBranchModal
		bind:this={addDependentBranchModal}
		{...addDependentBranchModalContext}
	/>
{/if}

<style lang="postcss">
	.branches-wrapper {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		margin: 12px 0;
	}
</style>
