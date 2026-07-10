<script lang="ts">
	import BranchBadge from "$components/branch/BranchBadge.svelte";
	import BranchHeader from "$components/branch/BranchHeader.svelte";
	import BranchHeaderContextMenu from "$components/branch/BranchHeaderContextMenu.svelte";
	import CIChecksBadge from "$components/forge/CIChecksBadge.svelte";
	import CreateReviewBox from "$components/forge/CreateReviewBox.svelte";
	import Dropzone from "$components/shared/Dropzone.svelte";
	import DropzoneOverlay from "$components/shared/DropzoneOverlay.svelte";
	import {
		BranchDropData,
		StartCommitDzHandler,
	} from "$lib/dragging/dropHandlers/branchDropHandler";
	import { MoveCommitDzHandler } from "$lib/dragging/dropHandlers/commitDropHandler";
	import { ReorderCommitDzHandler } from "$lib/dragging/stackingReorderDropzoneManager";
	import { FORGE_INFO_SERVICE } from "$lib/forge/forgeInfo.svelte";
	import { PR_SERVICE } from "$lib/forge/prService.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { UI_STATE } from "$lib/state/uiState.svelte";
	import { inject } from "@gitbutler/core/context";
	import { ReviewBadge, TestId } from "@gitbutler/ui";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";
	import type { BranchIconName } from "$lib/branches/branchIcon";
	import type { DropzoneHandler } from "$lib/dragging/handler";
	import type { PushStatus, Segment } from "@gitbutler/but-sdk";
	import type { Snippet } from "svelte";

	interface BranchCardProps {
		type: "normal-branch" | "stack-branch" | "pr-branch";
		projectId: string;
		branchName: string;
		isCommitting?: boolean;
		lineColor: string;
		readonly: boolean;
		first?: boolean;
		overflowHidden?: boolean;
	}

	interface NormalBranchProps extends BranchCardProps {
		type: "normal-branch";
		iconName: BranchIconName;
		selected: boolean;
		trackingBranch?: string;
		isTopBranch?: boolean;
		isNewBranch?: boolean;
		roundedBottom?: boolean;
		onclick?: () => void;
		disableClick?: boolean;
		branchContent: Snippet;
	}

	interface StackBranchProps extends BranchCardProps {
		type: "stack-branch";
		branchColor: string;
		iconName: BranchIconName;
		stackId?: string;
		laneId: string;
		selected: boolean;
		trackingBranch?: string;
		isNewBranch?: boolean;
		prNumber?: number;
		allOtherPrNumbersInStack: number[];
		reviewId?: string;
		pushStatus: PushStatus;
		isConflicted: boolean;
		applied?: boolean;
		contextMenu?: typeof BranchHeaderContextMenu;
		dropzones: DropzoneHandler[];
		numberOfCommits: number;
		numberOfUpstreamCommits: number;
		numberOfBranchesInStack: number;
		segment: Segment;
		branchIndex: number;
		parent: Segment | undefined;
		withForce: boolean;
		stackPrNumbers: (number | undefined)[];
		baseCommit?: string;
		onclick: () => void;
		disableClick?: boolean;
		menu?: Snippet<[{ rightClickTrigger: HTMLElement }]>;
		buttons?: Snippet;
		branchContent: Snippet;
		changedFiles?: Snippet;
	}

	interface PrBranchProps extends BranchCardProps {
		type: "pr-branch";
		selected: boolean;
		trackingBranch: string;
	}

	type Props = NormalBranchProps | StackBranchProps | PrBranchProps;

	let { projectId, branchName, lineColor, readonly, overflowHidden, ...args }: Props = $props();

	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const prService = inject(PR_SERVICE);
	const forgeInfoService = inject(FORGE_INFO_SERVICE);

	const forgeInfoQuery = $derived(forgeInfoService.get(projectId));
	const forgeInfo = $derived(forgeInfoQuery.response);
	const prUnit = $derived(forgeInfo?.unit);
	const checksEnabled = $derived(!!forgeInfo?.capabilities.checks);

	const [renameReference, referenceRename] = stackService.branchRename;

	const isUpdatingName = $derived(referenceRename.current.isLoading);
	const failedToUpdateName = $derived(referenceRename.current.isError);

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);

	const showPrCreation = $derived(
		exclusiveAction?.type === "create-pr" &&
			exclusiveAction.stackId === (args.type === "stack-branch" ? args.stackId : undefined) &&
			exclusiveAction.branchName === branchName,
	);

	const laneState = $derived(args.type === "stack-branch" ? uiState.lane(args.laneId) : undefined);
	const selection = $derived(laneState ? laneState.selection.current : undefined);
	const selected = $derived(selection?.branchName === branchName);
	const isPushed = $derived(!!args.trackingBranch);
	const isCommitTarget = $derived(
		exclusiveAction?.type === "commit" && exclusiveAction.branchName === branchName,
	);

	// Consolidated rounded bottom logic from both BranchCard and BranchHeader
	const isRoundedBottom = $derived.by(() => {
		// Empty branches being committed should be rounded
		if (args.isCommitting) {
			const isEmpty =
				(args.type === "stack-branch" || args.type === "normal-branch") && args.isNewBranch;
			if (isEmpty) return true;

			// Stack branches with no commits should be rounded when committing
			if (args.type === "stack-branch") {
				return args.numberOfCommits === 0;
			}
		}

		// For stack branches not committing, check if actions are visible and structural conditions
		if (args.type === "stack-branch" && !args.isCommitting) {
			const hasActions = args.buttons !== undefined || args.menu !== undefined;
			const structurallyRounded = args.numberOfCommits === 0 && args.numberOfUpstreamCommits === 0;
			return hasActions && structurallyRounded;
		}

		return false;
	});

	async function updateBranchName(title: string) {
		if (args.type === "stack-branch") {
			if (!args.stackId) return;
			// The backend re-normalizes, but we normalize here too so the optimistic selection
			// update in `branchRename` lands on the name the branch will actually have.
			const normalized = await stackService.normalizeBranchName(title);
			if (!normalized || normalized === branchName) return;
			await renameReference({
				projectId,
				refName: [...new TextEncoder().encode(`refs/heads/${branchName}`)],
				newName: normalized,
				laneId: args.laneId,
				branchName,
			});
		}
	}

	function getDropzoneOverlayLabel(handler: DropzoneHandler | undefined): string {
		if (handler instanceof MoveCommitDzHandler) return "Move here";
		if (handler instanceof ReorderCommitDzHandler) return "Reorder here";
		if (handler instanceof StartCommitDzHandler) return "Start commit";
		return "Drop here";
	}
</script>

<div
	class="branch-card"
	class:selected
	data-series-name={branchName}
	data-testid={TestId.BranchCard}
	style:overflow={overflowHidden ? "hidden" : undefined}
>
	{#if args.type === "stack-branch"}
		{@const moveHandler = args.stackId
			? new MoveCommitDzHandler(args.stackId, projectId, branchName)
			: undefined}

		<Dropzone
			handlers={args.first ? [moveHandler, ...args.dropzones].filter(isDefined) : args.dropzones}
		>
			{#snippet overlay({ hovered, activated, handler })}
				{@const label = getDropzoneOverlayLabel(handler)}
				<DropzoneOverlay {hovered} {activated} {label} />
			{/snippet}

			<BranchHeader
				{branchName}
				isEmpty={args.isNewBranch}
				selected={args.selected}
				draft={false}
				branchColor={args.branchColor}
				iconName={args.iconName}
				isCommitting={args.isCommitting}
				{isCommitTarget}
				commitId={args.baseCommit}
				onCommitGoesHereClick={() => {
					if (!args.stackId) return;
					projectState.exclusiveAction.set({
						type: "commit",
						stackId: args.stackId,
						branchName,
					});
				}}
				{updateBranchName}
				{isUpdatingName}
				failedMisserablyToUpdateBranchName={failedToUpdateName}
				roundedBottom={isRoundedBottom}
				{readonly}
				{isPushed}
				onclick={args.disableClick ? undefined : args.onclick}
				disableClick={args.disableClick}
				menu={args.menu}
				buttons={args.buttons}
				conflicts={args.isConflicted}
				{showPrCreation}
				changedFiles={args.changedFiles}
				dragArgs={{
					disabled: args.isConflicted || (args.type === "stack-branch" && args.applied === false),
					label: branchName,
					pushStatus: args.pushStatus,
					data:
						args.type === "stack-branch" && args.stackId
							? new BranchDropData(
									args.stackId,
									branchName,
									args.isConflicted,
									args.numberOfBranchesInStack,
									args.numberOfCommits,
									args.prNumber,
									args.allOtherPrNumbersInStack,
								)
							: undefined,
				}}
			>
				{#snippet emptyState()}
					<span class="branch-header__empty-state-span">This is an empty branch.</span>
					<span class="branch-header__empty-state-span">Click for details.</span>
					<br />
					Create or drag & drop commits here.
				{/snippet}

				{#snippet content()}
					<BranchBadge pushStatus={args.pushStatus} unstyled />

					{#if args.reviewId || args.prNumber}
						<span class="branch-header__divider">•</span>
						<div class="branch-header__review-badges">
							{#if args.prNumber}
								{@const prQuery = prService.get(projectId, args.prNumber, { forceRefetch: true })}
								{@const pr = prQuery.response}
								{@const mergeStatusQuery = prService.getMergeStatus(projectId, args.prNumber)}
								{@const prStatus = (() => {
									if (!pr) return "unknown";
									if (pr.mergedAt) return "merged";
									if (pr.closedAt) return "closed";
									if (pr.draft) return "draft";
									return "open";
								})()}
								<ReviewBadge
									testId={TestId.PRReviewBadge}
									type={prUnit?.abbr}
									forge={forgeInfo?.name}
									number={args.prNumber}
									status={prStatus}
								/>
								{#if pr && !pr.closedAt && checksEnabled && !pr.mergedAt}
									<CIChecksBadge
										{projectId}
										branchName={pr.sourceBranch}
										prUpdatedAt={pr.modifiedAt}
										mergeableState={mergeStatusQuery?.response?.mergeableState ?? undefined}
										isFork={pr.headRepoIsFork}
										isMerged={!!pr.mergedAt}
										onrefetch={() => {
											if (args.prNumber)
												prService.fetch(projectId, args.prNumber, { forceRefetch: true });
										}}
									/>
								{/if}
							{/if}
						</div>
					{/if}
				{/snippet}

				{#snippet prCreation()}
					<div class="review-wrapper" class:no-padding={uiState.global.useFloatingBox.current}>
						<CreateReviewBox
							{projectId}
							{branchName}
							stackId={args.stackId}
							segment={args.segment}
							branchIndex={args.branchIndex}
							parent={args.parent}
							withForce={args.withForce}
							stackPrNumbers={args.stackPrNumbers}
							prNumber={args.prNumber}
							oncancel={() => {
								projectState.exclusiveAction.set(undefined);
							}}
						/>
					</div>
				{/snippet}
			</BranchHeader>
		</Dropzone>
	{:else if args.type === "normal-branch"}
		<BranchHeader
			{branchName}
			isEmpty={args.isNewBranch}
			selected={args.selected}
			draft={false}
			branchColor={lineColor}
			iconName={args.iconName}
			{updateBranchName}
			{isUpdatingName}
			failedMisserablyToUpdateBranchName={failedToUpdateName}
			readonly
			{isPushed}
			onclick={args.disableClick ? undefined : args.onclick}
			disableClick={args.disableClick}
			roundedBottom={args.roundedBottom}
		>
			{#snippet emptyState()}
				<span class="branch-header__empty-state-span">There are no commits yet on this branch.</span
				>
			{/snippet}
		</BranchHeader>
	{:else if args.type === "pr-branch"}
		<BranchHeader
			{branchName}
			isEmpty
			selected={args.selected}
			draft={false}
			branchColor={lineColor}
			iconName="branch"
			{updateBranchName}
			{isUpdatingName}
			failedMisserablyToUpdateBranchName={failedToUpdateName}
			readonly
			isPushed
		/>
	{/if}

	{#if args.type === "stack-branch" || args.type === "normal-branch"}
		{#if args.branchContent}
			{@render args.branchContent()}
		{/if}
	{/if}
</div>

<style lang="postcss">
	.branch-card {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
	}

	.branch-header__divider {
		color: var(--text-3);
	}

	.branch-header__empty-state-span {
		text-wrap: nowrap;
	}

	.branch-header__review-badges {
		box-sizing: border-box;
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.review-wrapper {
		border-top: 1px solid var(--border-2);
		background-color: var(--bg-1);

		&:not(.no-padding) {
			padding: 12px;
		}
	}
</style>
