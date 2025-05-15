<script lang="ts">
	import CardOverlay from '$components/CardOverlay.svelte';
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import LineOverlay from '$components/LineOverlay.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import StackStickyButtons from '$components/StackStickyButtons.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BranchCommitList from '$components/v3/BranchCommitList.svelte';
	import BranchHeader from '$components/v3/BranchHeader.svelte';
	import BranchHeaderContextMenu, {
		type BranchHeaderContextItem
	} from '$components/v3/BranchHeaderContextMenu.svelte';
	import CommitContextMenu, {
		type CommitMenuContext
	} from '$components/v3/CommitContextMenu.svelte';
	import CommitGoesHere from '$components/v3/CommitGoesHere.svelte';
	import CommitRow from '$components/v3/CommitRow.svelte';
	import ConflictResolutionConfirmModal from '$components/v3/ConflictResolutionConfirmModal.svelte';
	import KebabButton from '$components/v3/KebabButton.svelte';
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
		AmendCommitWithHunkDzHandler,
		CommitDropData,
		SquashCommitDzHandler,
		type DzCommitData
	} from '$lib/commits/dropHandler';
	import { draggableCommitV3 } from '$lib/dragging/draggable';
	import {
		ReorderCommitDzHandler,
		StackingReorderDropzoneManagerFactory
	} from '$lib/dragging/stackingReorderDropzoneManager';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { ModeService } from '$lib/mode/modeService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext, inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import type { CommitStatusType } from '$lib/commits/commit';

	type Props = {
		isVerticalMode: boolean;
		projectId: string;
		stackId: string;
		active: boolean;
	};

	const { isVerticalMode, projectId, stackId, active }: Props = $props();
	const [stackService, uiState, baseBranchService, forge, modeService] = inject(
		StackService,
		UiState,
		BaseBranchService,
		DefaultForgeFactory,
		ModeService
	);

	const branchesResult = $derived(stackService.branches(projectId, stackId));

	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchResponse.current.data);
	const baseSha = $derived(base?.baseSha);

	const projectState = $derived(uiState.project(projectId));
	const drawer = $derived(projectState.drawerPage);
	const isCommitting = $derived(drawer.current === 'new-commit');

	const stackActive = $derived(stackId === projectState.stackId.current);
	const stackState = $derived(uiState.stack(stackId));
	const selection = $derived(stackState.selection.get());
	const selectedCommitId = $derived(selection.current?.commitId);
	const selectedBranchName = $derived(selection.current?.branchName);

	let newBranchModal = $state<ReturnType<typeof NewBranchModal>>();

	let conflictResolutionConfirmationModal =
		$state<ReturnType<typeof ConflictResolutionConfirmModal>>();

	async function handleUncommit(commitId: string, branchName: string) {
		await stackService.uncommit({ projectId, stackId, branchName, commitId: commitId });

		projectState.drawerPage.set(undefined);
		if (branchName) stackState.selection.set({ branchName, commitId: undefined });
	}

	function startCommitMessageEdition(branchName: string, commitId: string) {
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

	let headerMenuContext = $state<BranchHeaderContextItem>();
	let commitMenuContext = $state<CommitMenuContext>();

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
											{selected}
											{isNewBranch}
											{pushStatus}
											{isConflicted}
											{lastUpdatedAt}
											{reviewId}
											{prNumber}
											{active}
											isTopBranch={first}
											trackingBranch={branch.remoteTrackingBranch || undefined}
											readonly={!!branch.remoteTrackingBranch}
											onclick={() => {
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
											}}
										>
											{#snippet menu({ rightClickTrigger })}
												{@const data = {
													branch,
													prNumber,
													first: i === 0
												}}
												<KebabButton
													flat
													contextElement={rightClickTrigger}
													onclick={(element) =>
														(headerMenuContext = { data, position: { element } })}
													oncontext={(coords) =>
														(headerMenuContext = { data, position: { coords } })}
													contextElementSelected={selected}
													activated={branchName === headerMenuContext?.data.branch.name &&
														!!headerMenuContext.position.element}
												/>
											{/snippet}
										</BranchHeader>
									{/snippet}
									{#snippet commitList()}
										{#snippet commitReorderDz(dropzone: ReorderCommitDzHandler)}
											<Dropzone handlers={[dropzone]}>
												{#snippet overlay({ hovered, activated })}
													<LineOverlay {hovered} {activated} />
												{/snippet}
											</Dropzone>
										{/snippet}
										<BranchCommitList {projectId} {stackId} {branchName} {selectedCommitId}>
											{#snippet empty()}
												{#if isCommitting}
													<CommitGoesHere
														selected={stackActive && branchName === selectedBranchName}
														first
														last
														onclick={() => {
															uiState.stack(stackId).selection.set({
																branchName,
																commitId: branchDetails.baseCommit
															});
															projectState.stackId.set(stackId);
														}}
													/>
												{/if}
											{/snippet}
											{#snippet upstreamTemplate({ commit, first, lastCommit })}
												{@const selected =
													stackActive &&
													commit.id === selectedCommitId &&
													branchName === selectedBranchName}
												{@const commitId = commit.id}
												{#if !isCommitting}
													<CommitRow
														type="Remote"
														{stackId}
														{commitId}
														commitMessage={commit.message}
														createdAt={commit.createdAt}
														tooltip="Upstream"
														{branchName}
														{projectId}
														{first}
														lastCommit={lastCommit && localAndRemoteCommits.length === 0}
														{selected}
														{active}
														onclick={() => {
															uiState
																.stack(stackId)
																.selection.set({ branchName, commitId, upstream: true });
															uiState.project(projectId).drawerPage.set(undefined);
															projectState.stackId.set(stackId);
														}}
														disableCommitActions={false}
													/>
												{/if}
											{/snippet}
											{#snippet beforeLocalAndRemote()}
												{@render commitReorderDz(stackingReorderDropzoneManager.top(branch.name))}
											{/snippet}
											{#snippet localAndRemoteTemplate({
												commit,
												first,
												last,
												lastCommit,
												ancestorMostConflicted
											})}
												{@const commitId = commit.id}
												{@const selected =
													stackActive &&
													commit.id === selectedCommitId &&
													branchName === selectedBranchName}
												{#if isCommitting}
													{@const nothingSelectedButFirst = selectedCommitId === undefined && first}
													{@const selectedForCommit =
														stackActive &&
														(nothingSelectedButFirst || commit.id === selectedCommitId) &&
														((first && selectedBranchName === undefined) ||
															branchName === selectedBranchName)}
													<!-- Only commits to the base can be `last`, see next `CommitGoesHere`. -->
													<CommitGoesHere
														selected={selectedForCommit}
														{first}
														last={false}
														onclick={() => {
															projectState.stackId.set(stackId);
															uiState.stack(stackId).selection.set({ branchName, commitId });
														}}
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
													(newId) =>
														uiState.stack(stackId).selection.set({ branchName, commitId: newId }),
													uiState
												)}
												{@const squashHandler = new SquashCommitDzHandler({
													stackService,
													projectId,
													stackId,
													commit: dzCommit
												})}
												{@const hunkHandler = new AmendCommitWithHunkDzHandler({
													stackService,
													projectId,
													stackId,
													commit: dzCommit,
													// TODO: Use correct value!
													okWithForce: true,
													uiState
												})}
												{@const tooltip = commit.state.type}
												<Dropzone handlers={[amendHandler, squashHandler, hunkHandler]}>
													{#snippet overlay({ hovered, activated, handler })}
														{@const label =
															handler instanceof AmendCommitWithChangeDzHandler ||
															handler instanceof AmendCommitWithHunkDzHandler
																? 'Amend'
																: 'Squash'}
														<CardOverlay {hovered} {activated} {label} />
													{/snippet}
													<div
														use:draggableCommitV3={{
															disabled: false,
															label: commit.message.split('\n')[0],
															sha: commit.id.slice(0, 7),
															date: getTimeAgo(commit.createdAt),
															authorImgUrl: undefined,
															commitType: commit.state.type,
															data: new CommitDropData(
																stackId,
																{
																	id: commitId,
																	isRemote: !!branchDetails.remoteTrackingBranch,
																	hasConflicts:
																		isLocalAndRemoteCommit(commit) && commit.hasConflicts,
																	isIntegrated:
																		isLocalAndRemoteCommit(commit) &&
																		commit.state.type === 'Integrated'
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
															hasConflicts={commit.state.type === 'LocalAndRemote' &&
																commit.hasConflicts}
															diverged={commit.state.type === 'LocalAndRemote' &&
																commit.id !== commit.state.subject}
															createdAt={commit.createdAt}
															{stackId}
															{branchName}
															{projectId}
															{first}
															{lastCommit}
															{lastBranch}
															{selected}
															draggable
															{tooltip}
															{active}
															isOpen={commit.id === commitMenuContext?.data.commitId}
															onclick={() => {
																const stackState = uiState.stack(stackId);
																stackState.selection.set({ branchName, commitId });
																uiState.project(projectId).drawerPage.set(undefined);
																projectState.stackId.set(stackId);
															}}
															disableCommitActions={false}
														>
															{#snippet menu({ rightClickTrigger })}
																{@const data = {
																	stackId,
																	commitId,
																	commitMessage: commit.message,
																	commitStatus: commit.state.type,
																	commitUrl: forge.current.commitUrl(commitId),
																	onUncommitClick: () => handleUncommit(commit.id, branch.name),
																	onEditMessageClick: () =>
																		startCommitMessageEdition(branchName, commit.id),
																	onPatchEditClick: () =>
																		handleEditPatch({
																			commitId: commit.id,
																			type: commit.state.type,
																			hasConflicts: hasConflicts(commit),
																			isAncestorMostConflicted:
																				ancestorMostConflicted?.id === commit.id
																		})
																}}
																<KebabButton
																	flat
																	contextElement={rightClickTrigger}
																	onclick={(element) => {
																		commitMenuContext = {
																			position: { element },
																			data
																		};
																	}}
																	oncontext={(coords) =>
																		(commitMenuContext = {
																			position: { coords },
																			data
																		})}
																	contextElementSelected={selected}
																	activated={commit.id === commitMenuContext?.data.commitId &&
																		!!commitMenuContext.position.element}
																/>
															{/snippet}
														</CommitRow>
													</div>
												</Dropzone>
												{@render commitReorderDz(
													stackingReorderDropzoneManager.belowCommit(branch.name, commit.id)
												)}
												{#if isCommitting && last}
													<CommitGoesHere
														{first}
														{last}
														selected={stackActive &&
															selectedCommitId === baseSha &&
															branchName === selectedBranchName}
														onclick={() => {
															uiState
																.stack(stackId)
																.selection.set({ branchName, commitId: baseSha });
															projectState.stackId.set(stackId);
														}}
													/>
												{/if}
											{/snippet}
										</BranchCommitList>
									{/snippet}
								</BranchCard>
							{/snippet}
						</ReduxResult>
					{/each}
				</div>
			</ScrollableContainer>
			<StackStickyButtons {isVerticalMode}>
				<PushButton flex="1" {projectId} {stackId} multipleBranches={branches.length > 1} />
				<PublishButton flex="2" {projectId} {stackId} {branches} />
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

{#if headerMenuContext}
	<BranchHeaderContextMenu {projectId} {stackId} bind:context={headerMenuContext} />
{/if}

{#if commitMenuContext}
	<CommitContextMenu {projectId} bind:context={commitMenuContext} />
{/if}

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
	.branches-wrapper {
		flex: 1;
		display: flex;
		flex-direction: column;
		padding: 12px;
	}
</style>
