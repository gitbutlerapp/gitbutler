<script lang="ts">
	import BranchBadge from '$components/BranchBadge.svelte';
	import BranchDividerLine from '$components/BranchDividerLine.svelte';
	import BranchHeader from '$components/BranchHeader.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import CardOverlay from '$components/CardOverlay.svelte';
	import ChecksPolling from '$components/ChecksPolling.svelte';
	import CreateReviewBox from '$components/CreateReviewBox.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import PrNumberUpdater from '$components/PrNumberUpdater.svelte';
	import { BranchDropData } from '$lib/branches/dropHandler';
	import { MoveCommitDzHandler } from '$lib/commits/dropHandler';
	import { ReorderCommitDzHandler } from '$lib/dragging/stackingReorderDropzoneManager';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { ReviewBadge, TestId } from '@gitbutler/ui';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { DropzoneHandler } from '$lib/dragging/handler';
	import type { PushStatus } from '$lib/stacks/stack';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	interface BranchCardProps {
		type: 'normal-branch' | 'stack-branch' | 'pr-branch';
		projectId: string;
		branchName: string;
		isCommitting?: boolean;
		lineColor: string;
		readonly: boolean;
		first?: boolean;
	}

	interface NormalBranchProps extends BranchCardProps {
		type: 'normal-branch';
		iconName: keyof typeof iconsJson;
		selected: boolean;
		trackingBranch?: string;
		lastUpdatedAt?: number;
		isTopBranch?: boolean;
		isNewBranch?: boolean;
		onclick: () => void;
		branchContent: Snippet;
		codegenRow?: Snippet;
	}

	interface StackBranchProps extends BranchCardProps {
		type: 'stack-branch';
		iconName: keyof typeof iconsJson;
		stackId?: string;
		laneId: string;
		selected: boolean;
		trackingBranch?: string;
		isNewBranch?: boolean;
		prNumber?: number;
		allOtherPrNumbersInStack: number[];
		reviewId?: string;
		pushStatus: PushStatus;
		lastUpdatedAt?: number;
		isConflicted: boolean;
		contextMenu?: typeof BranchHeaderContextMenu;
		dropzones: DropzoneHandler[];
		numberOfCommits: number;
		numberOfUpstreamCommits: number;
		numberOfBranchesInStack: number;
		hasCodegenRow?: boolean;
		baseCommit?: string;
		onclick: () => void;
		menu?: Snippet<[{ rightClickTrigger: HTMLElement }]>;
		buttons?: Snippet;
		branchContent: Snippet;
		codegenRow?: Snippet;
	}

	interface PrBranchProps extends BranchCardProps {
		type: 'pr-branch';
		selected: boolean;
		trackingBranch: string;
		lastUpdatedAt: number;
	}

	type Props = NormalBranchProps | StackBranchProps | PrBranchProps;

	let { projectId, branchName, lineColor, readonly, ...args }: Props = $props();

	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);

	const prService = $derived(forge.current.prService);
	const prUnit = $derived(prService?.unit);

	const [updateName, nameUpdate] = stackService.updateBranchName;

	const projectState = $derived(uiState.project(projectId));
	const exclusiveAction = $derived(projectState.exclusiveAction.current);

	const showPrCreation = $derived(
		exclusiveAction?.type === 'create-pr' &&
			exclusiveAction.stackId === (args.type === 'stack-branch' ? args.stackId : undefined) &&
			exclusiveAction.branchName === branchName
	);

	const laneState = $derived(args.type === 'stack-branch' ? uiState.lane(args.laneId) : undefined);
	const selection = $derived(laneState ? laneState.selection.current : undefined);
	const selected = $derived(selection?.branchName === branchName);
	const isPushed = $derived(!!args.trackingBranch);
	const isCommitTarget = $derived(
		exclusiveAction?.type === 'commit' && exclusiveAction.branchName === branchName
	);

	// Consolidated rounded bottom logic from both BranchCard and BranchHeader
	const isRoundedBottom = $derived.by(() => {
		// Empty branches being committed should be rounded
		if (args.isCommitting) {
			const isEmpty =
				(args.type === 'stack-branch' || args.type === 'normal-branch') && args.isNewBranch;
			if (isEmpty) return true;

			// Stack branches with codegen row or no commits should be rounded when committing
			if (args.type === 'stack-branch') {
				return args.hasCodegenRow || args.numberOfCommits === 0;
			}
		}

		// For stack branches not committing, check if actions are visible and structural conditions
		if (args.type === 'stack-branch' && !args.isCommitting) {
			const hasActions = args.buttons !== undefined || args.menu !== undefined;
			const structurallyRounded = args.hasCodegenRow || args.numberOfCommits === 0;
			return hasActions && structurallyRounded;
		}

		return false;
	});

	async function updateBranchName(title: string) {
		if (args.type === 'stack-branch') {
			if (!args.stackId) return;
			updateName({
				projectId,
				stackId: args.stackId,
				laneId: args.laneId,
				branchName,
				newName: title
			});
		}
	}
</script>

{#if ((args.type === 'stack-branch' && !args.first) || (args.type === 'normal-branch' && !args.first)) && lineColor}
	<BranchDividerLine {lineColor} />
{/if}

<div
	class="branch-card"
	class:selected
	data-series-name={branchName}
	data-testid={TestId.BranchCard}
>
	{#if args.type === 'stack-branch'}
		{@const moveHandler = args.stackId
			? new MoveCommitDzHandler(stackService, args.stackId, projectId, uiState)
			: undefined}
		{#if !args.prNumber && args.stackId}
			<PrNumberUpdater {projectId} stackId={args.stackId} {branchName} />
		{/if}

		<Dropzone
			handlers={args.first ? [moveHandler, ...args.dropzones].filter(isDefined) : args.dropzones}
		>
			{#snippet overlay({ hovered, activated, handler })}
				{@const label =
					handler instanceof MoveCommitDzHandler
						? 'Move here'
						: handler instanceof ReorderCommitDzHandler
							? 'Reorder here'
							: 'Start commit'}
				<CardOverlay {hovered} {activated} {label} />
			{/snippet}

			<BranchHeader
				{branchName}
				isEmpty={args.isNewBranch}
				selected={args.selected}
				draft={false}
				{lineColor}
				isCommitting={args.isCommitting}
				{isCommitTarget}
				commitId={args.baseCommit}
				onCommitGoesHereClick={() => {
					if (!args.stackId) return;
					projectState.exclusiveAction.set({
						type: 'commit',
						stackId: args.stackId,
						branchName,
						parentCommitId: args.baseCommit
					});
				}}
				iconName={args.iconName}
				{updateBranchName}
				isUpdatingName={nameUpdate.current.isLoading}
				failedMisserablyToUpdateBranchName={nameUpdate.current.isError}
				roundedBottom={isRoundedBottom}
				{readonly}
				{isPushed}
				onclick={args.onclick}
				menu={args.menu}
				conflicts={args.isConflicted}
				{showPrCreation}
				dragArgs={{
					disabled: args.isConflicted,
					label: branchName,
					pushStatus: args.pushStatus,
					viewportId: 'board-viewport',
					data:
						args.type === 'stack-branch' && args.stackId
							? new BranchDropData(
									args.stackId,
									branchName,
									args.isConflicted,
									args.numberOfBranchesInStack,
									args.numberOfCommits,
									args.prNumber,
									args.allOtherPrNumbersInStack
								)
							: undefined
				}}
			>
				{#snippet buttons()}
					{#if args.buttons}
						{@render args.buttons()}
					{/if}
				{/snippet}

				{#snippet emptyState()}
					<span class="branch-header__empty-state-span">This is an empty branch.</span>
					<span class="branch-header__empty-state-span">Click for details.</span>
					<br />
					Create or drag & drop commits here.
				{/snippet}

				{#snippet content()}
					<BranchBadge pushStatus={args.pushStatus} unstyled />

					<span class="branch-header__divider">•</span>

					{#if args.lastUpdatedAt}
						<span class="branch-header__item">
							{getTimeAgo(new Date(args.lastUpdatedAt))}
						</span>
					{/if}

					{#if args.reviewId || args.prNumber}
						<span class="branch-header__divider">•</span>
						<div class="branch-header__review-badges">
							{#if args.prNumber}
								{@const prQuery = prService?.get(args.prNumber, { forceRefetch: true })}
								{@const pr = prQuery?.response}
								<ReviewBadge type={prUnit?.abbr} number={args.prNumber} status="unknown" />
								{#if pr && !pr.closedAt && forge.current.checks && pr.state === 'open'}
									<ChecksPolling
										{projectId}
										branchName={pr.sourceBranch}
										isFork={pr.fork}
										isMerged={pr.merged}
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
							oncancel={() => {
								projectState.exclusiveAction.set(undefined);
							}}
						/>
					</div>
				{/snippet}
			</BranchHeader>
		</Dropzone>
	{:else if args.type === 'normal-branch'}
		<BranchHeader
			{branchName}
			isEmpty={args.isNewBranch}
			selected={args.selected}
			draft={false}
			{lineColor}
			iconName={args.iconName}
			{updateBranchName}
			isUpdatingName={nameUpdate.current.isLoading}
			failedMisserablyToUpdateBranchName={nameUpdate.current.isError}
			readonly
			{isPushed}
			onclick={args.onclick}
		>
			{#snippet emptyState()}
				<span class="branch-header__empty-state-span">There are no commits yet on this branch.</span
				>
			{/snippet}
			{#snippet content()}
				{#if args.lastUpdatedAt}
					<span class="branch-header__item">
						{getTimeAgo(new Date(args.lastUpdatedAt))}
					</span>
				{/if}
			{/snippet}
		</BranchHeader>
	{:else if args.type === 'pr-branch'}
		<BranchHeader
			{branchName}
			isEmpty
			selected={args.selected}
			draft={false}
			{lineColor}
			iconName="branch-remote"
			{updateBranchName}
			isUpdatingName={nameUpdate.current.isLoading}
			failedMisserablyToUpdateBranchName={nameUpdate.current.isError}
			readonly
			isPushed
		>
			{#snippet content()}
				{#if args.lastUpdatedAt}
					<span class="branch-header__item">
						{getTimeAgo(new Date(args.lastUpdatedAt))}
					</span>
				{/if}
			{/snippet}
		</BranchHeader>
	{/if}

	{#if args.type === 'stack-branch' && args.hasCodegenRow && args.codegenRow}
		<BranchDividerLine {lineColor} height="0.375rem" />
		{@render args.codegenRow()}
		{#if args.numberOfCommits > 0 || args.numberOfUpstreamCommits > 0}
			<BranchDividerLine {lineColor} height="0.375rem" />
		{/if}
	{/if}

	{#if args.type === 'stack-branch' || args.type === 'normal-branch'}
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

	.branch-header__item {
		color: var(--clr-text-2);
		white-space: nowrap;
	}

	.branch-header__divider {
		color: var(--clr-text-3);
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
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);

		&:not(.no-padding) {
			padding: 12px;
		}
	}
</style>
