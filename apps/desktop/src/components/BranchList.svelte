<script lang="ts">
	import AddDependentBranchModal, {
		type AddDependentBranchModalProps
	} from '$components/AddDependentBranchModal.svelte';
	import BranchCard from '$components/BranchCard.svelte';
	import BranchCommitList from '$components/BranchCommitList.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import BranchInsertion from '$components/BranchInsertion.svelte';
	import CodegenRow from '$components/CodegenRow.svelte';
	import ConflictResolutionConfirmModal from '$components/ConflictResolutionConfirmModal.svelte';
	import NestedChangedFiles from '$components/NestedChangedFiles.svelte';
	import PushButton from '$components/PushButton.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import { getColorFromCommitState, getIconFromCommitState } from '$components/lib';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { focusClaudeInput } from '$lib/codegen/focusClaudeInput';
	import { currentStatus } from '$lib/codegen/messages';
	import { projectDisableCodegen } from '$lib/config/config';
	import { REORDER_DROPZONE_FACTORY } from '$lib/dragging/stackingReorderDropzoneManager';
	import { editPatch } from '$lib/editMode/editPatchUtils';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { MODE_SERVICE } from '$lib/mode/modeService';
	import { createBranchSelection } from '$lib/selection/key';
	import { type BranchDetails } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { URL_SERVICE } from '$lib/utils/url';
	import { ensureValue } from '$lib/utils/validation';
	import { inject } from '@gitbutler/core/context';
	import { Button, Modal, TestId } from '@gitbutler/ui';
	import { getForgeLogo } from '@gitbutler/ui/utils/getForgeLogo';
	import { QueryStatus } from '@reduxjs/toolkit/query';
	import { tick } from 'svelte';
	import type { CommitStatusType } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		stackId?: string;
		laneId: string;
		branches: BranchDetails[];
		active: boolean;
		onclick?: () => void;
		onFileClick?: (index: number) => void;
	};

	const { projectId, branches, stackId, laneId, active, onclick, onFileClick }: Props = $props();
	const stackService = inject(STACK_SERVICE);
	const uiState = inject(UI_STATE);
	const modeService = inject(MODE_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const urlService = inject(URL_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);

	// Component is read-only when stackId is undefined
	const isReadOnly = $derived(!stackId);

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
	const codegenDisabled = $derived(projectDisableCodegen(projectId));

	let conflictResolutionConfirmationModal =
		$state<ReturnType<typeof ConflictResolutionConfirmModal>>();

	async function handleUncommit(commitId: string, branchName: string) {
		await stackService.uncommit({
			projectId,
			stackId: ensureValue(stackId),
			branchName,
			commitId: commitId
		});
	}

	function startEditingCommitMessage(branchName: string, commitId: string) {
		laneState.selection.set({ branchName, commitId, previewOpen: true });
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
		if (isReadOnly) return;
		if (args.type === 'LocalAndRemote' && args.hasConflicts && !args.isAncestorMostConflicted) {
			conflictResolutionConfirmationModal?.show();
			return;
		}
		await editPatch({
			modeService,
			commitId: args.commitId,
			stackId: ensureValue(stackId),
			projectId
		});
	}

	const selectedCommit = $derived(
		selectedCommitId ? stackService.commitDetails(projectId, selectedCommitId) : undefined
	);

	$effect(() => {
		if (selectedCommit && selectedCommit.result.status === QueryStatus.rejected) {
			const branchName = selection.current?.branchName;
			if (branchName) {
				selection.set({ branchName, commitId: undefined, previewOpen: false });
			}
		}
	});

	const stackingReorderDropzoneManagerFactory = inject(REORDER_DROPZONE_FACTORY);
	const stackingReorderDropzoneManager = $derived(
		stackingReorderDropzoneManagerFactory.build(
			projectId,
			laneId,
			branches.map((s) => ({ name: s.name, commitIds: s.commits.map((p) => p.id) }))
		)
	);

	const canPublishPR = $derived(forge.current.authenticated);
	const baseBranchNameResponse = $derived(baseBranchService.baseBranchShortName(projectId));
	const baseBranchName = $derived(baseBranchNameResponse.response);
</script>

<div class="branches-wrapper">
	{#each branches as branch, i}
		{@const branchName = branch.name}
		{@const localAndRemoteCommits = stackService.commits(projectId, stackId, branchName)}
		{@const upstreamOnlyCommits = stackService.upstreamCommits(projectId, stackId, branchName)}
		{@const branchDetailsQuery = stackService.branchDetails(projectId, stackId, branchName)}
		{@const commitQuery = stackService.commitAt(projectId, stackId, branchName, 0)}
		{@const prQuery = branch.prNumber ? forge.current.prService?.get(branch.prNumber) : undefined}

		{@const first = i === 0}

		<ReduxResult
			{projectId}
			{stackId}
			result={combineResults(
				localAndRemoteCommits.result,
				upstreamOnlyCommits.result,
				branchDetailsQuery.result,
				commitQuery.result
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
					selection?.current.commitId === undefined &&
					!selection?.current.codegen}
				{@const pushStatus = branchDetails.pushStatus}
				{@const isConflicted = branchDetails.isConflicted}
				{@const reviewId = branch.reviewId || undefined}
				{@const prNumber = branch.prNumber || undefined}
				{@const allOtherPrNumbersInStack = branches
					.filter((b) => b.name !== branchName)
					.map((b) => b.prNumber)
					.filter((n): n is number => n !== undefined)}
				{@const codegenSelected =
					selection?.current?.branchName === branchName &&
					selection?.current.commitId === undefined &&
					selection?.current.codegen}
				{@const codegenQuery = stackId
					? claudeCodeService.messages({ projectId, stackId })
					: undefined}
				{#if stackId}
					<BranchInsertion
						{projectId}
						{stackId}
						{branchName}
						{lineColor}
						{isCommitting}
						{baseBranchName}
						{stackService}
						prService={forge.current.prService}
						isFirst={firstBranch}
					/>
				{/if}
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
					hasCodegenRow={firstBranch &&
						stackId !== undefined &&
						codegenQuery?.response &&
						codegenQuery.response.length > 0}
					{reviewId}
					{prNumber}
					{allOtherPrNumbersInStack}
					numberOfCommits={localAndRemoteCommits.length}
					numberOfUpstreamCommits={upstreamOnlyCommits.length}
					numberOfBranchesInStack={branches.length}
					baseCommit={branchDetails.baseCommit}
					dropzones={[stackingReorderDropzoneManager.top(branchName)]}
					trackingBranch={branch.remoteTrackingBranch ?? undefined}
					readonly={!!branch.remoteTrackingBranch}
					onclick={() => {
						const currentSelection = uiState.lane(laneId).selection.current;
						// Toggle: if this branch is already selected, clear the selection
						if (
							currentSelection?.branchName === branchName &&
							!currentSelection.codegen &&
							!currentSelection?.commitId
						) {
							uiState.lane(laneId).selection.set(undefined);
						} else {
							uiState.lane(laneId).selection.set({ branchName, previewOpen: true });
						}
						onclick?.();
					}}
				>
					{#snippet buttons()}
						{#if first}
							<Button
								icon="new-dep-branch"
								size="tag"
								kind="outline"
								tooltip={isReadOnly ? 'Read-only mode' : 'Create new branch'}
								onclick={async () => {
									addDependentBranchModalContext = {
										projectId,
										stackId: ensureValue(stackId)
									};

									await tick();
									addDependentBranchModal?.show();
								}}
								disabled={isReadOnly}
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
								{@const prUrl = prQuery?.response?.htmlUrl}
								<Button
									size="tag"
									kind="outline"
									shrinkable
									disabled={!prUrl}
									onclick={() => {
										if (prUrl) {
											urlService.openExternalUrl(prUrl);
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
						{#if !$codegenDisabled && first && codegenQuery?.response?.length === 0}
							<Button
								icon="ai-small"
								style="gray"
								size="tag"
								tooltip="New Codegen Session"
								onclick={async () => {
									if (!stackId) return;
									laneState?.selection.set({ branchName, codegen: true, previewOpen: true });
									setTimeout(() => {
										focusClaudeInput(stackId);
									}, 100);
								}}
							/>
						{/if}
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

					{#snippet codegenRow()}
						{#if firstBranch && stackId}
							{#if codegenQuery?.response && codegenQuery.response.length > 0}
								{@const stackActive = claudeCodeService.isStackActive(projectId, stackId)}
								{@const status = currentStatus(
									codegenQuery.response || [],
									stackActive.response || false
								)}
								<CodegenRow
									{projectId}
									{branchName}
									{stackId}
									{status}
									selected={codegenSelected}
									{onclick}
								/>
							{/if}
						{/if}
					{/snippet}

					{#snippet changedFiles()}
						{#if selected}
							{@const changesQuery = stackService.branchChanges({
								projectId,
								stackId,
								branch: branchName
							})}
							<ReduxResult {projectId} {stackId} result={changesQuery.result}>
								{#snippet children(result, { projectId, stackId })}
									<NestedChangedFiles
										title="All Changes"
										{projectId}
										{stackId}
										draggableFiles
										autoselect
										foldedByDefault
										selectionId={createBranchSelection({
											stackId: stackId,
											branchName,
											remote: undefined
										})}
										persistId={`branch-${branchName}`}
										changes={result.changes}
										stats={result.stats}
										allowUnselect={false}
										onFileClick={(index) => {
											// Ensure the branch is selected so the preview shows it
											const currentSelection = laneState.selection.current;
											if (
												currentSelection?.branchName !== branchName ||
												currentSelection?.commitId !== undefined
											) {
												laneState.selection.set({ branchName, previewOpen: true });
											}
											onFileClick?.(index);
										}}
									/>
								{/snippet}
							</ReduxResult>
						{/if}
					{/snippet}

					{#snippet branchContent()}
						<BranchCommitList
							{lastBranch}
							{projectId}
							{stackId}
							{laneId}
							{branchName}
							{branchDetails}
							{stackingReorderDropzoneManager}
							roundedTop={firstBranch &&
								stackId !== undefined &&
								codegenQuery?.response &&
								codegenQuery.response.length > 0}
							{active}
							{handleUncommit}
							{startEditingCommitMessage}
							{onclick}
							{onFileClick}
						/>
					{/snippet}
				</BranchCard>
			{/snippet}
		</ReduxResult>
	{/each}
</div>

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
