<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import StackStickyButtons from '$components/StackStickyButtons.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeaderContextMenu, {
		type BranchHeaderContextItem
	} from '$components/v3/BranchHeaderContextMenu.svelte';
	import ConflictResolutionConfirmModal from '$components/v3/ConflictResolutionConfirmModal.svelte';
	import KebabButton from '$components/v3/KebabButton.svelte';
	import NewBranchModal from '$components/v3/NewBranchModal.svelte';
	import PublishButton from '$components/v3/PublishButton.svelte';
	import PushButton from '$components/v3/PushButton.svelte';
	import { getColorFromCommitState, getIconFromCommitState } from '$components/v3/lib';
	import { StackingReorderDropzoneManagerFactory } from '$lib/dragging/stackingReorderDropzoneManager';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import type { CommitStatusType } from '$lib/commits/commit';

	type Props = {
		isVerticalMode: boolean;
		projectId: string;
		stackId: string;
		active: boolean;
	};

	const { isVerticalMode, projectId, stackId, active }: Props = $props();
	const [stackService, uiState, modeService] = inject(StackService, UiState, ModeService);

	const branchesResult = $derived(stackService.branches(projectId, stackId));

	const projectState = $derived(uiState.project(projectId));
	const drawer = $derived(projectState.drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const stackActive = $derived(stackId === projectState.stackId.current);
	const stackState = $derived(uiState.stack(stackId));
	const selection = $derived(stackState.selection.get());
	const selectedCommitId = $derived(selection.current?.commitId);

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();

	let conflictResolutionConfirmationModal =
		$state<ReturnType<typeof ConflictResolutionConfirmModal>>();

	async function handleUncommit(commitId: string, branchName: string) {
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commitId });
	}

	function startEditingCommitMessage(branchName: string, commitId: string) {
		stackState.selection.set({ branchName, commitId });
		projectState.drawerPage.set(undefined);
		projectState.editingCommitMessage.set(true);
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

	function handleBranchClick(branchName: string, headCommit: string) {
		uiState.project(projectId).stackId.set(stackId);
		if (isCommitting) {
			uiState.stack(stackId).selection.set({
				branchName,
				commitId: headCommit
			});
			projectState.stackId.set(stackId);
		} else {
			uiState.stack(stackId).selection.set({ branchName });
			uiState.project(projectId).drawerPage.set('branch');
		}
	}

	let headerMenuContext = $state<BranchHeaderContextItem>();

	const stackingReorderDropzoneManagerFactory = getContext(StackingReorderDropzoneManagerFactory);
</script>

<div class="wrapper">
	<ReduxResult {stackId} {projectId} result={branchesResult.current}>
		{#snippet children(branches, { stackId, projectId })}
			{@const stackingReorderDropzoneManager = stackingReorderDropzoneManagerFactory.build(
				stackId,
				branches.map((s) => ({ name: s.name, commitIds: s.commits.map((p) => p.id) }))
			)}
			<ScrollableContainer>
				<div class="branches-wrapper">
					{#each branches as branch, i}
						{@const branchName = branch.name}
						{@const localAndRemoteCommits = stackService.commits(projectId, stackId, branchName)}
						{@const upstreamOnlyCommits = stackService.upstreamCommits(
							projectId,
							stackId,
							branchName
						)}
						{@const branchDetailsResult = stackService.branchDetails(
							projectId,
							stackId,
							branchName
						)}
						{@const commitResult = stackService.commitAt(projectId, stackId, branchName, 0)}
						{@const first = i === 0}
						{@const headCommit = branch.commits[0]?.id || branch.baseCommit}

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
							{#snippet children([
								localAndRemoteCommits,
								upstreamOnlyCommits,
								branchDetails,
								commit
							])}
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
									stackActive &&
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
									{active}
									trackingBranch={branch.remoteTrackingBranch ?? undefined}
									readonly={!!branch.remoteTrackingBranch}
									onclick={() => {
										handleBranchClick(branchName, headCommit);
									}}
								>
									{#snippet menu({ rightClickTrigger })}
										{@const data = {
											branch,
											prNumber,
											first,
											stackLength: branches.length
										}}
										<KebabButton
											flat
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
											{lastBranch}
											{active}
											{projectId}
											{stackId}
											{branchName}
											{selectedCommitId}
											{branchDetails}
											{stackingReorderDropzoneManager}
											{handleUncommit}
											{startEditingCommitMessage}
											{handleEditPatch}
										/>
									{/snippet}
								</BranchCard>
							{/snippet}
						</ReduxResult>
					{/each}
				</div>
			</ScrollableContainer>
			<StackStickyButtons {isVerticalMode}>
				<PushButton flex="1" {projectId} {stackId} multipleBranches={branches.length > 1} />
				{@const reviewCreationInOpen =
					drawer.current === 'review' && stackId === projectState.stackId.current}
				<PublishButton flex="2" {projectId} {stackId} {branches} {reviewCreationInOpen} />
			</StackStickyButtons>
		{/snippet}
	</ReduxResult>
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

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
	.branches-wrapper {
		display: flex;
		flex: 1;
		flex-direction: column;
		padding: 12px;
	}
</style>
