<script lang="ts">
	import AddDependentBranchModal, {
		type AddDependentBranchModalProps
	} from '$components/AddDependentBranchModal.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeaderContextMenu, {
		type BranchHeaderContextItem
	} from '$components/v3/BranchHeaderContextMenu.svelte';
	import ConflictResolutionConfirmModal from '$components/v3/ConflictResolutionConfirmModal.svelte';
	import KebabButton from '$components/v3/KebabButton.svelte';
	import NewBranchModal from '$components/v3/NewBranchModal.svelte';
	import PushButton from '$components/v3/PushButton.svelte';
	import { getColorFromCommitState, getIconFromCommitState } from '$components/v3/lib';
	import { StackingReorderDropzoneManagerFactory } from '$lib/dragging/stackingReorderDropzoneManager';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { IntelligentScrollingService } from '$lib/intelligentScrolling/service';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { openExternalUrl } from '$lib/utils/url';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { getContext, inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { QueryStatus } from '@reduxjs/toolkit/query';
	import { tick } from 'svelte';
	import type { CommitStatusType } from '$lib/commits/commit';
	import type { BranchDetails } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stackId: string;
		branches: BranchDetails[];
		focusedStackId?: string;
		onselect?: () => void;
	};

	const { projectId, branches, stackId, focusedStackId, onselect }: Props = $props();
	const [stackService, uiState, modeService, forge, intelligentScrollingService] = inject(
		StackService,
		UiState,
		ModeService,
		DefaultForgeFactory,
		IntelligentScrollingService
	);

	const [insertBlankCommitInBranch, commitInsertion] = stackService.insertBlankCommit;

	let addDependentBranchModalContext = $state<AddDependentBranchModalProps>();
	let addDependentBranchModal = $state<AddDependentBranchModal>();

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);
	const isCommitting = $derived(
		exclusiveAction?.type === 'commit' && exclusiveAction?.stackId === stackId
	);
	const stackState = $derived(uiState.stack(stackId));
	const selection = $derived(stackState.selection);
	const selectedCommitId = $derived(selection.current?.commitId);

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();

	let conflictResolutionConfirmationModal =
		$state<ReturnType<typeof ConflictResolutionConfirmModal>>();

	async function handleUncommit(commitId: string, branchName: string) {
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commitId });
	}

	function startEditingCommitMessage(branchName: string, commitId: string) {
		stackState.selection.set({ branchName, commitId });
		projectState.exclusiveAction.set({ type: 'edit-commit-message', commitId });
	}

	async function handleEditPatch(args: {
		commitId: string;
		type: CommitStatusType;
		hasConflicts: boolean;
		isAncestorMostConflicted: boolean;
	}) {
		if (args.type === 'LocalAndRemote' && args.hasConflicts && !args.isAncestorMostConflicted) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		modeService!.enterEditMode(args.commitId, stackId);
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

	let headerMenuContext = $state<BranchHeaderContextItem>();

	const stackingReorderDropzoneManagerFactory = getContext(StackingReorderDropzoneManagerFactory);
	const stackingReorderDropzoneManager = $derived(
		stackingReorderDropzoneManagerFactory.build(
			stackId,
			branches.map((s) => ({ name: s.name, commitIds: s.commits.map((p) => p.id) }))
		)
	);

	const canPublishPR = $derived(forge.current.authenticated);
</script>

<div class="wrapper">
	<div class="branches-wrapper">
		{#each branches as branch, i}
			{@const branchName = branch.name}
			{@const localAndRemoteCommits = stackService.commits(projectId, stackId, branchName)}
			{@const upstreamOnlyCommits = stackService.upstreamCommits(projectId, stackId, branchName)}
			{@const branchDetailsResult = stackService.branchDetails(projectId, stackId, branchName)}
			{@const commitResult = stackService.commitAt(projectId, stackId, branchName, 0)}
			{@const prResult = branch.prNumber
				? forge.current.prService?.get(branch.prNumber)
				: undefined}

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
							if (selection.current?.branchName === branchName && !selection.current.commitId) {
								uiState.stack(stackId).selection.set(undefined);
							} else {
								uiState.stack(stackId).selection.set({ branchName });
								intelligentScrollingService.show(projectId, stackId, 'details');
							}
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
									await insertBlankCommitInBranch({
										projectId,
										stackId,
										commitOid: undefined,
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
										addDependentBranchModalContext = {
											projectId,
											stackId
										};

										await tick();
										addDependentBranchModal?.show();
									}}
								/>
							{/if}

							{#if stackState?.action.current !== 'review' && canPublishPR && !isNewBranch}
								{#if !branch.prNumber}
									<Button
										size="tag"
										kind="outline"
										shrinkable
										icon="github-small"
										onclick={(e) => {
											e.stopPropagation();
											stackState?.action.set('review');
										}}
										testId={TestId.CreateReviewButton}
									>
										Create PR
									</Button>
								{:else}
									{@const prUrl = prResult?.current.data?.htmlUrl}
									<Button
										size="tag"
										kind="outline"
										shrinkable
										disabled={!prUrl}
										icon="view-pr-browser"
										onclick={() => {
											if (prUrl) {
												openExternalUrl(prUrl);
											}
										}}>View PR</Button
									>
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
							<KebabButton
								contextElement={rightClickTrigger}
								onclick={(element) => (headerMenuContext = { data, position: { element } })}
								oncontext={(coords) => (headerMenuContext = { data, position: { coords } })}
								contextElementSelected={selected}
								activated={branchName === headerMenuContext?.data.branch.name &&
									!!headerMenuContext.position.element}
							/>
						{/snippet}

						{#snippet branchContent()}
							<BranchCommitList
								{firstBranch}
								{lastBranch}
								active={focusedStackId === stackId}
								{projectId}
								{stackId}
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

<BranchHeaderContextMenu {projectId} {stackId} bind:context={headerMenuContext} />

{#if addDependentBranchModalContext}
	<AddDependentBranchModal
		bind:this={addDependentBranchModal}
		{...addDependentBranchModalContext}
	/>
{/if}

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
	}

	.branches-wrapper {
		display: flex;
		flex: 1;
		flex-direction: column;
		padding: 12px;
	}
</style>
