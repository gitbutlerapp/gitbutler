<script lang="ts">
	import AddDependentBranchModal, {
		type AddDependentBranchModalProps,
	} from "$components/branch/AddDependentBranchModal.svelte";
	import BranchCard from "$components/branch/BranchCard.svelte";
	import BranchDividerLine from "$components/branch/BranchDividerLine.svelte";
	import BranchHeaderContextMenu from "$components/branch/BranchHeaderContextMenu.svelte";
	import BranchReorderDropzone from "$components/branch/BranchReorderDropzone.svelte";
	import ChangedFilesPanel from "$components/files/ChangedFilesPanel.svelte";
	import PushButton from "$components/forge/PushButton.svelte";
	import { getColorFromCommitState, getIconFromCommitState } from "$components/lib";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import BranchCommitList from "$components/views/BranchCommitList.svelte";
	import { URL_SERVICE } from "$lib/backend/url";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { StartCommitDzHandler } from "$lib/dragging/dropHandlers/branchDropHandler";
	import { REORDER_DROPZONE_FACTORY } from "$lib/dragging/stackingReorderDropzoneManager";
	import { DEFAULT_FORGE_FACTORY } from "$lib/forge/forgeFactory.svelte";
	import { createBranchSelection } from "$lib/selection/key";
	import { getStackContext } from "$lib/stacks/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { ensureValue } from "$lib/utils/validation";
	import { inject } from "@gitbutler/core/context";
	import { Button, TestId } from "@gitbutler/ui";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	import { tick } from "svelte";
	import type { Segment } from "@gitbutler/but-sdk";

	type Props = {
		branches: Segment[];
	};

	const { branches }: Props = $props();

	const controller = getStackContext();
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const urlService = inject(URL_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const projectId = $derived(controller.projectId);
	const stackId = $derived(controller.stackId);
	const laneId = $derived(controller.laneId);

	let addDependentBranchModalContext = $state<AddDependentBranchModalProps>();
	let addDependentBranchModal = $state<AddDependentBranchModal>();

	const selection = $derived(controller.selection);
	const selectedCommitId = $derived(controller.commitId);
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
			branches
				.map((s) =>
					s.refName
						? { name: s.refName.displayName, commitIds: s.commits.map((p) => p.id) }
						: undefined,
				)
				.filter((branch): branch is { name: string; commitIds: string[] } => branch !== undefined),
		),
	);

	const canPublishPR = $derived(forge.current.authenticated);
	const baseBranchNameResponse = $derived(baseBranchService.baseBranchShortName(projectId));
	const baseBranchName = $derived(baseBranchNameResponse.response);
</script>

<div class="branches-wrapper">
	{#each branches as branch, i}
		{@const branchName = branch.refName?.displayName}
		{@const branchLabel = branchName ?? "Unnamed segment"}
		{@const remoteTrackingBranch = branch.remoteTrackingRefName
			? new TextDecoder().decode(new Uint8Array(branch.remoteTrackingRefName.fullNameBytes))
			: undefined}
		{@const prNumber = branch.metadata?.review.pullRequest ?? undefined}
		{@const reviewId = branch.metadata?.review.reviewId ?? undefined}
		{@const prQuery = prNumber ? forge.current.prService?.get(prNumber) : undefined}
		{@const commit = branch.commits.at(0)}

		{@const first = i === 0}

		{@const firstBranch = i === 0}
		{@const lastBranch = i === branches.length - 1}
		{@const iconName = getIconFromCommitState(commit?.id, commit?.state)}
		{@const lineColor = commit
			? getColorFromCommitState(
					commit.state.type,
					commit.state.type === "LocalAndRemote" && commit.id !== commit.state.subject,
				)
			: "var(--commit-local)"}
		{@const isNewBranch = branch.commitsOnRemote.length === 0 && branch.commits.length === 0}
		{@const selected =
			selection?.current?.branchName === branchName && selection?.current?.commitId === undefined}
		{@const allOtherPrNumbersInStack = branches
			.filter((b) => b.refName?.displayName !== branchName)
			.map((b) => b.metadata?.review.pullRequest)
			.filter((n): n is number => n !== undefined && n !== null)}
		{@const isConflicted = branch.commits.some((commit) => commit.hasConflicts)}
		{@const startCommittingDz = branchName
			? new StartCommitDzHandler(projectId, stackId, branchName)
			: undefined}
		{#if stackId && branchName}
			<BranchReorderDropzone
				{projectId}
				{stackId}
				{branchName}
				{lineColor}
				isCommitting={controller.isCommitting}
				{baseBranchName}
				prService={forge.current.prService}
				isFirst={firstBranch}
			/>
		{:else if !firstBranch}
			<BranchDividerLine {lineColor} />
		{/if}
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
						<ChangedFilesPanel
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

		{#snippet buttons()}
			{#if first && branchName}
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

			{#if canPublishPR && !isNewBranch && branchName}
				{#if !prNumber}
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
			{#if branchName}
				<PushButton
					{branchName}
					{projectId}
					{stackId}
					multipleBranches={branches.length > 1}
					isFirstBranchInStack={firstBranch}
					isLastBranchInStack={lastBranch}
				/>
			{/if}
		{/snippet}

		{#snippet menu({ rightClickTrigger }: { rightClickTrigger?: HTMLElement })}
			{#if branchName}
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
			{/if}
		{/snippet}

		<BranchCard
			type="stack-branch"
			{projectId}
			stackId={branchName ? stackId : undefined}
			{laneId}
			branchName={branchLabel}
			{lineColor}
			{first}
			isCommitting={controller.isCommitting}
			{iconName}
			{selected}
			{isNewBranch}
			pushStatus={branch.pushStatus}
			{isConflicted}
			{reviewId}
			{prNumber}
			{allOtherPrNumbersInStack}
			numberOfCommits={branch.commits.length}
			numberOfUpstreamCommits={branch.commitsOnRemote.length}
			numberOfBranchesInStack={branches.length}
			baseCommit={branch.base ?? undefined}
			dropzones={branchName && startCommittingDz
				? [stackingReorderDropzoneManager.top(branchName), startCommittingDz]
				: []}
			trackingBranch={remoteTrackingBranch}
			readonly={!branchName || !!remoteTrackingBranch}
			disableClick={!branchName}
			onclick={() => {
				if (!branchName) return;
				const currentSelection = controller.selection.current;
				// Toggle: if this branch is already selected, clear the selection
				if (currentSelection?.branchName === branchName && !currentSelection?.commitId) {
					controller.selection.set(undefined);
				} else {
					controller.selection.set({ branchName, previewOpen: true });
				}
				controller.clearWorktreeSelection();
			}}
			changedFiles={branch.refName ? changedFiles : undefined}
			buttons={branch.refName ? buttons : undefined}
			menu={branch.refName ? menu : undefined}
		>
			{#snippet branchContent()}
				<BranchCommitList
					{lastBranch}
					{branchName}
					segment={branch}
					{stackingReorderDropzoneManager}
				/>
			{/snippet}
		</BranchCard>
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
