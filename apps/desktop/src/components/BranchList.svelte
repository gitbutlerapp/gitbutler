<script lang="ts">
	import AddDependentBranchModal, {
		type AddDependentBranchModalProps,
	} from "$components/AddDependentBranchModal.svelte";
	import BranchCard from "$components/BranchCard.svelte";
	import BranchCommitList from "$components/BranchCommitList.svelte";
	import BranchHeaderContextMenu from "$components/BranchHeaderContextMenu.svelte";
	import BranchInsertion from "$components/BranchInsertion.svelte";
	import CodegenRow from "$components/CodegenRow.svelte";
	import NestedChangedFiles from "$components/NestedChangedFiles.svelte";
	import PushButton from "$components/PushButton.svelte";
	import ReduxResult from "$components/ReduxResult.svelte";
	import { getColorFromCommitState, getIconFromCommitState } from "$components/lib";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { StartCommitDzHandler } from "$lib/branches/dropHandler";
	import { CLAUDE_CODE_SERVICE } from "$lib/codegen/claude";
	import { focusClaudeInput } from "$lib/codegen/focusClaudeInput";
	import { currentStatus } from "$lib/codegen/messages";
	import { projectDisableCodegen } from "$lib/config/config";
	import { REORDER_DROPZONE_FACTORY } from "$lib/dragging/stackingReorderDropzoneManager";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { createBranchSelection } from "$lib/selection/key";
	import { getStackContext } from "$lib/stack/stackController.svelte";
	import { type BranchDetails } from "$lib/stacks/stack";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { combineResults } from "$lib/state/helpers";
	import { URL_SERVICE } from "$lib/utils/url";
	import { ensureValue } from "$lib/utils/validation";
	import { inject } from "@gitbutler/core/context";
	import { Button, TestId } from "@gitbutler/ui";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	import { tick } from "svelte";

	type Props = {
		branches: BranchDetails[];
	};

	const { branches }: Props = $props();

	const controller = getStackContext();
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const urlService = inject(URL_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const claudeCodeService = inject(CLAUDE_CODE_SERVICE);
	const projectId = $derived(controller.projectId);
	const stackId = $derived(controller.stackId);
	const laneId = $derived(controller.laneId);

	let addDependentBranchModalContext = $state<AddDependentBranchModalProps>();
	let addDependentBranchModal = $state<AddDependentBranchModal>();

	const selection = $derived(controller.selection);
	const selectedCommitId = $derived(controller.commitId);
	const codegenDisabled = $derived(projectDisableCodegen(projectId));

	const selectedCommit = $derived(
		selectedCommitId ? stackService.commitDetails(projectId, selectedCommitId) : undefined,
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
			branches.map((s) => ({ name: s.name, commitIds: s.commits.map((p) => p.id) })),
		),
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
				commitQuery.result,
			)}
		>
			{#snippet children(
				[localAndRemoteCommits, upstreamOnlyCommits, branchDetails, commit],
				{ projectId, stackId },
			)}
				{@const firstBranch = i === 0}
				{@const lastBranch = i === branches.length - 1}
				{@const iconName = getIconFromCommitState(commit?.id, commit?.state)}
				{@const lineColor = commit
					? getColorFromCommitState(
							commit.state.type,
							commit.state.type === "LocalAndRemote" && commit.id !== commit.state.subject,
						)
					: "var(--clr-commit-local)"}
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
				{@const startCommittingDz = new StartCommitDzHandler(projectId, stackId, branchName)}
				{#if stackId}
					<BranchInsertion
						{projectId}
						{stackId}
						{branchName}
						{lineColor}
						isCommitting={controller.isCommitting}
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
					isCommitting={controller.isCommitting}
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
					dropzones={[stackingReorderDropzoneManager.top(branchName), startCommittingDz]}
					trackingBranch={branch.remoteTrackingBranch ?? undefined}
					readonly={!!branch.remoteTrackingBranch}
					onclick={() => {
						const currentSelection = controller.selection.current;
						// Toggle: if this branch is already selected, clear the selection
						if (
							currentSelection?.branchName === branchName &&
							!currentSelection.codegen &&
							!currentSelection?.commitId
						) {
							controller.selection.set(undefined);
						} else {
							controller.selection.set({ branchName, previewOpen: true });
						}
						controller.clearWorktreeSelection();
					}}
				>
					{#snippet buttons()}
						{#if first}
							<Button
								icon="stack-plus"
								size="tag"
								kind="outline"
								tooltip={controller.isReadOnly ? "Read-only mode" : "Create new branch"}
								onclick={async () => {
									addDependentBranchModalContext = {
										projectId,
										stackId: ensureValue(stackId),
									};

									await tick();
									addDependentBranchModal?.show();
								}}
								disabled={controller.isReadOnly}
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
										controller.projectState.exclusiveAction.set({
											type: "create-pr",
											stackId,
											branchName,
										});
									}}
									testId={TestId.CreateReviewButton}
									disabled={!!controller.exclusiveAction}
									icon="pr-plus"
								>
									{`Create ${forge.current.name === "gitlab" ? "MR" : "PR"}`}
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
									icon="arrow-up-righ"
								>
									{`View ${forge.current.name === "gitlab" ? "MR" : "PR"}`}
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
								icon="ai"
								style="gray"
								size="tag"
								tooltip="New Codegen Session"
								onclick={async () => {
									if (!stackId) return;
									controller.selection.set({ branchName, codegen: true, previewOpen: true });
									focusClaudeInput(stackId);
								}}
							/>
						{/if}
					{/snippet}

					{#snippet menu({ rightClickTrigger })}
						{@const data = {
							branch,
							prNumber,
							first,
							stackLength: branches.length,
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
									stackActive.response || false,
								)}
								<CodegenRow
									{projectId}
									{branchName}
									{stackId}
									{status}
									selected={codegenSelected}
									onclick={() => controller.clearWorktreeSelection()}
								/>
							{/if}
						{/if}
					{/snippet}

					{#snippet changedFiles()}
						<!--
							Based on anecdotal evidence the type for `items` seems incorrect. It's
							likely that during some kind of unmount event items can become `undefined`
							due to some subtle reactivity condition, causing `branchChanges` to be
							called without a branch name.
						-->
						{#if selected && branchName}
							{@const changesQuery = stackService.branchChanges({
								projectId,
								stackId,
								branch: branchName,
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
											remote: undefined,
										})}
										persistId={`branch-${branchName}`}
										changes={result.changes}
										stats={result.stats}
										allowUnselect={false}
										visibleRange={controller.visibleRange}
										onFileClick={(index) => {
											// Ensure the branch is selected so the preview shows it
											const currentSelection = controller.selection.current;
											if (
												currentSelection?.branchName !== branchName ||
												currentSelection?.commitId !== undefined
											) {
												controller.selection.set({ branchName, previewOpen: true });
											}
											controller.jumpToIndex(index);
										}}
									/>
								{/snippet}
							</ReduxResult>
						{/if}
					{/snippet}

					{#snippet branchContent()}
						<BranchCommitList
							{lastBranch}
							{branchName}
							{branchDetails}
							{stackingReorderDropzoneManager}
							roundedTop={firstBranch &&
								stackId !== undefined &&
								codegenQuery?.response &&
								codegenQuery.response.length > 0}
						/>
					{/snippet}
				</BranchCard>
			{/snippet}
		</ReduxResult>
	{/each}
</div>

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
	}
</style>
