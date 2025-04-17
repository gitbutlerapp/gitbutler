<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import SeriesHeaderContextMenuContents from '$components/SeriesHeaderContextMenuContents.svelte';
	import StackStickyButtons from '$components/StackStickyButtons.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import CommitContextMenu from '$components/v3/CommitContextMenu.svelte';
	import CommitGoesHere from '$components/v3/CommitGoesHere.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import ConflictResolutionConfirmModal from '$components/v3/ConflictResolutionConfirmModal.svelte';
	import NewBranchModal from '$components/v3/NewBranchModal.svelte';
	import PublishButton from '$components/v3/PublishButton.svelte';
	import PushButton from '$components/v3/PushButton.svelte';
	import {
		getColorFromCommitState,
		getIconFromCommitState,
		hasConflicts,
		isLocalAndRemoteCommit,
		isUpstreamCommit
	} from '$components/v3/lib';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import {
		AmendCommitWithChangeDzHandler,
		CommitDropData,
		SquashCommitDzHandler,
		type DzCommitData
	} from '$lib/commits/dropHandler';
	import { draggableCommit } from '$lib/dragging/draggable';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { CommitStatusType } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		stackId: string;
	};

	const { projectId, stackId }: Props = $props();
	const [stackService, uiState, baseBranchService, forge] = inject(
		StackService,
		UiState,
		BaseBranchService,
		DefaultForgeFactory
	);

	const branchesResult = $derived(stackService.branches(projectId, stackId));

	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchResponse.current.data);
	const baseSha = $derived(base?.baseSha);

	const drawer = $derived(uiState.project(projectId).drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const selection = $derived(uiState.stack(stackId).selection.get());
	const selectedCommitId = $derived(selection.current?.commitId);
	const selectedBranchName = $derived(selection.current?.branchName);

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();

	let conflictResolutionConfirmationModal =
		$state<ReturnType<typeof ConflictResolutionConfirmModal>>();

	async function handleUncommit(commitId: string, branchName: string) {
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commitId });
	}

	function openCommitMessageModal() {
		// TODO: Implement openCommitMessageModal
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
		// modeService!.enterEditMode(args.commitId, stackId);
	}
</script>

<ReduxResult {stackId} {projectId} result={branchesResult.current}>
	{#snippet children(branches, { stackId, projectId })}
		{#each branches as branch, i}
			{@const branchName = branch.name}
			{@const localAndRemoteCommits = stackService.commits(projectId, stackId, branchName)}
			{@const upstreamOnlyCommits = stackService.upstreamCommits(projectId, stackId, branchName)}
			{@const branchDetailsResult = stackService.branchDetails(projectId, stackId, branchName)}
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
				{#snippet children([localAndRemoteCommits, upstreamOnlyCommits, branchDetails, commit])}
					{@const branchType = commit?.state.type || 'LocalOnly'}
					{@const iconName = getIconFromCommitState(commit?.id, commit?.state)}
					{@const lineColor = commit
						? getColorFromCommitState(
								commit.state.type,
								commit.state.type === 'LocalAndRemote' && commit.id !== commit.state.subject
							)
						: 'var(--clr-commit-local)'}
					{@const isNewBranch =
						upstreamOnlyCommits.length === 0 && localAndRemoteCommits.length === 0}
					{@const selected = selection?.current?.branchName === branchName}
					<BranchCard
						type="stack-branch"
						{projectId}
						{stackId}
						{branchName}
						{lineColor}
						{first}
						{isCommitting}
					>
						{#snippet header()}
							{@const pushStatus = branchDetails.pushStatus}
							{@const isConflicted = branchDetails.isConflicted}
							{@const lastUpdatedAt = branchDetails.lastUpdatedAt}
							{@const reviewId = branch.reviewId || undefined}
							{@const prNumber = branch.prNumber || undefined}
							<BranchHeader
								type="stack-branch"
								{branchName}
								{projectId}
								{stackId}
								{lineColor}
								{iconName}
								{isCommitting}
								{selected}
								{isNewBranch}
								{pushStatus}
								{isConflicted}
								{lastUpdatedAt}
								{reviewId}
								{prNumber}
								isTopBranch={first}
								trackingBranch={branch.remoteTrackingBranch || undefined}
								readonly={!!branch.remoteTrackingBranch}
								onclick={() => {
									if (isCommitting) {
										uiState.stack(stackId).selection.set({
											branchName,
											commitId: headCommit
										});
									} else {
										uiState.stack(stackId).selection.set({ branchName });
										uiState.project(projectId).drawerPage.set('branch');
									}
								}}
							>
								{#snippet menu({ showDeleteBranchModal, showBranchRenameModal })}
									{@const forgeBranch = branch.remoteTrackingBranch
										? forge.current?.branch(branch.remoteTrackingBranch)
										: undefined}
									<SeriesHeaderContextMenuContents
										{projectId}
										{stackId}
										{branchType}
										branchName={branch.name}
										seriesCount={branches.length}
										isTopBranch={first}
										descriptionOption={false}
										onGenerateBranchName={() => {
											throw new Error('Not implemented!');
										}}
										onAddDependentSeries={() => newBranchModal?.show(branchName)}
										onOpenInBrowser={() => {
											const url = forgeBranch?.url;
											if (url) openExternalUrl(url);
										}}
										isPushed={!!branch.remoteTrackingBranch}
										{showBranchRenameModal}
										{showDeleteBranchModal}
									/>
								{/snippet}
							</BranchHeader>
						{/snippet}
						{#snippet commitList()}
							<BranchCommitList {projectId} {stackId} {branchName} {selectedCommitId}>
								{#snippet empty()}
									{#if isCommitting}
										<CommitGoesHere
											selected={branchName === selectedBranchName}
											first
											last
											onclick={() =>
												uiState.stack(stackId).selection.set({
													branchName,
													commitId: branchDetails.baseCommit
												})}
										/>
									{/if}
								{/snippet}
								{#snippet upstreamTemplate({ commit, first, lastCommit, selected })}
									{@const commitId = commit.id}
									{#if !isCommitting}
										<CommitRow
											type={'Remote'}
											{stackId}
											{commitId}
											commitMessage={commit.message}
											createdAt={commit.createdAt}
											tooltip={'Upstream'}
											{branchName}
											{projectId}
											{first}
											lastCommit={lastCommit && localAndRemoteCommits.length === 0}
											{selected}
											onclick={() => {
												uiState
													.stack(stackId)
													.selection.set({ branchName, commitId, upstream: true });
												uiState.project(projectId).drawerPage.set(undefined);
											}}
											disableCommitActions={false}
										/>
									{/if}
								{/snippet}
								{#snippet localAndRemoteTemplate({
									commit,
									first,
									last,
									lastCommit,
									selectedCommitId
								})}
									{@const commitId = commit.id}
									{@const selected =
										commit.id === selectedCommitId && branchName === selectedBranchName}
									{#if isCommitting}
										<!-- Only commits to the base can be `last`, see next `CommitGoesHere`. -->
										<CommitGoesHere
											{selected}
											{first}
											last={false}
											onclick={() => uiState.stack(stackId).selection.set({ branchName, commitId })}
										/>
									{/if}
									{@const dzCommit: DzCommitData = {
										id: commit.id,
										isRemote: isUpstreamCommit(commit),
										isIntegrated: isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated',
										hasConflicts: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
									}}
									{@const amendHandler = new AmendCommitWithChangeDzHandler(
										projectId,
										stackService,
										stackId,
										dzCommit,
										(newId) => uiState.stack(stackId).selection.set({ branchName, commitId: newId })
									)}
									{@const squashHandler = new SquashCommitDzHandler({
										stackService,
										projectId,
										stackId,
										commit: dzCommit
									})}
									{@const tooltip = commit.state.type}
									<Dropzone handlers={[amendHandler, squashHandler]}>
										{#snippet overlay({ hovered, activated, handler })}
											{@const label =
												handler instanceof AmendCommitWithChangeDzHandler ? 'Amend' : 'Squash'}
											<CardOverlay {hovered} {activated} {label} />
										{/snippet}
										<div
											use:draggableCommit={{
												disabled: false,
												label: commit.message.split('\n')[0],
												sha: commit.id.slice(0, 7),
												date: getTimeAgo(commit.createdAt),
												authorImgUrl: undefined,
												commitType: 'LocalAndRemote',
												data: new CommitDropData(
													stackId,
													{
														id: commitId,
														isRemote: !!branchDetails.remoteTrackingBranch,
														hasConflicts: isLocalAndRemoteCommit(commit) && commit.hasConflicts,
														isIntegrated:
															isLocalAndRemoteCommit(commit) && commit.state.type === 'Integrated'
													},
													false,
													branchName
												),
												viewportId: 'board-viewport'
											}}
										>
											<CommitRow
												commitId={commit.id}
												commitMessage={commit.message}
												type={commit.state.type}
												hasConflicts={commit.state.type === 'LocalAndRemote' && commit.hasConflicts}
												diverged={commit.state.type === 'LocalAndRemote' &&
													commit.id !== commit.state.subject}
												createdAt={commit.createdAt}
												{stackId}
												{branchName}
												{projectId}
												{first}
												{lastCommit}
												lastBranch={last}
												{selected}
												draggable
												{tooltip}
												onclick={() => {
													const stackState = uiState.stack(stackId);
													stackState.selection.set({ branchName, commitId });
													uiState.project(projectId).drawerPage.set(undefined);
												}}
												disableCommitActions={false}
											>
												{#snippet menu({ close })}
													<CommitContextMenu
														{close}
														{projectId}
														{stackId}
														{commitId}
														commitMessage={commit.message}
														commitStatus={commit.state.type}
														commitUrl={forge.current.commitUrl(commitId)}
														onUncommitClick={() => handleUncommit(commit.id, branch.name)}
														onEditMessageClick={openCommitMessageModal}
														onPatchEditClick={() =>
															handleEditPatch({
																commitId: commit.id,
																type: commit.state.type,
																hasConflicts: hasConflicts(commit),
																isAncestorMostConflicted: false // TODO: Fix this.
															})}
													/>
												{/snippet}
											</CommitRow>
										</div>
									</Dropzone>
									{#if isCommitting && last}
										<CommitGoesHere
											{first}
											{last}
											selected={selectedCommitId === baseSha && branchName === selectedBranchName}
											onclick={() =>
												uiState.stack(stackId).selection.set({ branchName, commitId: baseSha })}
										/>
									{/if}
								{/snippet}
							</BranchCommitList>
						{/snippet}
					</BranchCard>
				{/snippet}
			</ReduxResult>
		{/each}
		<StackStickyButtons>
			<PushButton flex="1" {projectId} {stackId} multipleBranches={branches.length > 1} />
			<PublishButton flex="2" {projectId} {stackId} {branches} />
		</StackStickyButtons>
	{/snippet}
</ReduxResult>

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
	{#snippet children()}
		<div>
			<p>It's generally better to start resolving conflicts from the bottom up.</p>
			<br />
			<p>Are you sure you want to resolve conflicts for this commit?</p>
		</div>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit">Yes</Button>
	{/snippet}
</Modal>

<style lang="postcss">
</style>
