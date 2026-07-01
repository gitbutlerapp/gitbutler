<script lang="ts">
	import AddDependentBranchModal, {
		type AddDependentBranchModalProps,
	} from "$components/branch/AddDependentBranchModal.svelte";
	import BranchCard from "$components/branch/BranchCard.svelte";
	import BranchDividerLine from "$components/branch/BranchDividerLine.svelte";
	import BranchHeaderContextMenu from "$components/branch/BranchHeaderContextMenu.svelte";
	import BranchReorderDropzone from "$components/branch/BranchReorderDropzone.svelte";
	import LandBranchModal from "$components/branch/LandBranchModal.svelte";
	import ChangedFilesPanel from "$components/files/ChangedFilesPanel.svelte";
	import PushButton from "$components/forge/PushButton.svelte";
	import {
		getColorFromBranchPushStatus,
		getColorFromCommitState,
		getIconFromBranchPushStatus,
	} from "$components/lib";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import BranchCommitList from "$components/views/BranchCommitList.svelte";
	import { URL_SERVICE } from "$lib/backend/url";
	import { BASE_BRANCH_SERVICE } from "$lib/baseBranch/baseBranchService.svelte";
	import { projectLandDirectly } from "$lib/config/config";
	import { StartCommitDzHandler } from "$lib/dragging/dropHandlers/branchDropHandler";
	import { REORDER_DROPZONE_FACTORY } from "$lib/dragging/stackingReorderDropzoneManager";
	import { useForgeAuth } from "$lib/forge/forgeAuth.svelte";
	import { FORGE_INFO_SERVICE, prUrl } from "$lib/forge/forgeInfo.svelte";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
	import { createBranchSelection } from "$lib/selection/key";
	import { precomputeStack, segmentContext } from "$lib/stacks/segmentContext";
	import { getStackContext } from "$lib/stacks/stackController.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { ensureValue } from "$lib/utils/validation";
	import { inject } from "@gitbutler/core/context";
	import { reactive } from "@gitbutler/shared/reactiveUtils.svelte";
	import { Button, TestId } from "@gitbutler/ui";
	import { QueryStatus } from "@reduxjs/toolkit/query";
	import { tick } from "svelte";
	import type { Segment } from "@gitbutler/but-sdk";

	type Props = {
		segments: Segment[];
	};

	const { segments }: Props = $props();

	const controller = getStackContext();
	const stackService = inject(STACK_SERVICE);
	const prService = inject(PR_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);
	const urlService = inject(URL_SERVICE);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const projectId = $derived(controller.projectId);
	const stackId = $derived(controller.stackId);
	const laneId = $derived(controller.laneId);
	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const auth = useForgeAuth(reactive(() => projectId));

	let addDependentBranchModalContext = $state<AddDependentBranchModalProps>();
	let addDependentBranchModal = $state<AddDependentBranchModal>();
	let landBranchModal = $state<LandBranchModal>();

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
			segments
				.map((s) =>
					s.refName
						? { name: s.refName.displayName, commitIds: s.commits.map((p) => p.id) }
						: undefined,
				)
				.filter((branch): branch is { name: string; commitIds: string[] } => branch !== undefined),
		),
	);

	const canPublishPR = $derived(auth.authenticated.current);
	const landDirectly = $derived(projectLandDirectly(projectId));
	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const baseBranchName = $derived(baseBranchResponse.response?.shortName);
	const landTargetName = $derived(baseBranchResponse.response?.branchName);

	// Compute stack-wide derived values once per render so the per-iteration
	// segmentContext() call below stays O(1) instead of O(n) per index.
	const stackPrecomputed = $derived(precomputeStack(segments));
</script>

<div class="branches-wrapper">
	{#each segments as segment, i}
		{@const ctx = segmentContext(segments, i, stackPrecomputed)}
		{@const branchName = segment.refName?.displayName}
		{@const branchLabel = branchName ?? "Unnamed segment"}
		{@const remoteTrackingBranch = segment.remoteTrackingRefName
			? new TextDecoder().decode(new Uint8Array(segment.remoteTrackingRefName.fullNameBytes))
			: undefined}
		{@const prNumber = segment.metadata?.review.pullRequest ?? undefined}
		{@const reviewId = segment.metadata?.review.reviewId ?? undefined}
		{@const commit = segment.commits.at(0)}

		{@const first = i === 0}

		{@const firstBranch = i === 0}
		{@const lastBranch = i === segments.length - 1}
		{@const iconName = getIconFromBranchPushStatus(segment.pushStatus)}
		{@const branchColor = getColorFromBranchPushStatus(segment.pushStatus)}
		{@const lineColor = commit
			? getColorFromCommitState(
					commit.state.type,
					commit.state.type === "LocalAndRemote" && commit.id !== commit.state.subject,
				)
			: "var(--commit-local)"}
		{@const isNewBranch = segment.commitsOnRemote.length === 0 && segment.commits.length === 0}
		{@const selected =
			selection?.current?.branchName === branchName && selection?.current?.commitId === undefined}
		{@const allOtherPrNumbersInStack = segments
			.filter((s) => s.refName?.displayName !== branchName)
			.map((s) => s.metadata?.review.pullRequest)
			.filter((n): n is number => n !== undefined && n !== null)}
		{@const isConflicted = segment.commits.some((commit) => commit.hasConflicts)}
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
				{prService}
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

			{#if $landDirectly}
				{#if lastBranch && !isNewBranch && branchName}
					<Button
						size="tag"
						kind="outline"
						shrinkable
						onclick={(e) => {
							e.stopPropagation();
							landBranchModal?.show(branchName);
						}}
						disabled={!!controller.exclusiveAction || isConflicted}
						tooltip={isConflicted
							? "Resolve conflicts before landing"
							: "Land directly into the target branch"}
						icon="branch-merge"
					>
						Land
					</Button>
				{/if}
			{:else if canPublishPR && !isNewBranch && branchName}
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
						{`Create ${forgeInfo?.unit.abbr ?? "PR"}`}
					</Button>
				{:else}
					{@const externalPrUrl = forgeInfo ? prUrl(forgeInfo, prNumber) : undefined}
					<Button
						size="tag"
						kind="outline"
						shrinkable
						disabled={!externalPrUrl}
						onclick={() => {
							if (externalPrUrl) {
								urlService.openExternalUrl(externalPrUrl);
							}
						}}
						icon="arrow-up-righ"
					>
						{`View ${forgeInfo?.unit.abbr ?? "PR"}`}
					</Button>
				{/if}
			{/if}
			{#if branchName}
				<PushButton
					{branchName}
					{projectId}
					{stackId}
					{segment}
					withForce={ctx.withForce}
					multipleBranches={segments.length > 1}
					isFirstBranchInStack={firstBranch}
					isLastBranchInStack={lastBranch}
				/>
			{/if}
		{/snippet}

		{#snippet menu({ rightClickTrigger }: { rightClickTrigger?: HTMLElement })}
			{#if branchName}
				{@const data = {
					segment,
					prNumber,
					first,
					stackLength: segments.length,
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
			{branchColor}
			stackId={branchName ? stackId : undefined}
			{laneId}
			branchName={branchLabel}
			{lineColor}
			{first}
			isCommitting={controller.isCommitting}
			{iconName}
			{selected}
			{isNewBranch}
			pushStatus={segment.pushStatus}
			{isConflicted}
			{reviewId}
			{prNumber}
			{allOtherPrNumbersInStack}
			numberOfCommits={segment.commits.length}
			numberOfUpstreamCommits={segment.commitsOnRemote.length}
			numberOfBranchesInStack={segments.length}
			{segment}
			branchIndex={ctx.branchIndex}
			parent={ctx.parent}
			withForce={ctx.withForce}
			stackPrNumbers={ctx.stackPrNumbers}
			baseCommit={segment.base ?? undefined}
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
			changedFiles={segment.refName ? changedFiles : undefined}
			buttons={segment.refName ? buttons : undefined}
			menu={segment.refName ? menu : undefined}
		>
			{#snippet branchContent()}
				<BranchCommitList {lastBranch} {branchName} {segment} {stackingReorderDropzoneManager} />
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

<LandBranchModal bind:this={landBranchModal} {projectId} targetBranchName={landTargetName} />

<style lang="postcss">
	.branches-wrapper {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
	}
</style>
