<script lang="ts">
	import { goto } from '$app/navigation';
	import BranchBadge from '$components/BranchBadge.svelte';
	import BranchDividerLine from '$components/BranchDividerLine.svelte';
	import BranchHeader from '$components/BranchHeader.svelte';
	import BranchHeaderContextMenu from '$components/BranchHeaderContextMenu.svelte';
	import CardOverlay from '$components/CardOverlay.svelte';
	import ChecksPolling from '$components/ChecksPolling.svelte';
	import ClaudeSessionDescriptor from '$components/ClaudeSessionDescriptor.svelte';
	import CreateReviewBox from '$components/CreateReviewBox.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import PrNumberUpdater from '$components/PrNumberUpdater.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';

	import { CodegenRuleDropData, CodegenRuleDropHandler } from '$lib/codegen/dropzone';
	import { MoveCommitDzHandler } from '$lib/commits/dropHandler';
	import { DRAG_STATE_SERVICE } from '$lib/dragging/dragStateService.svelte';
	import { draggableChips } from '$lib/dragging/draggable';
	import { DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import { ReorderCommitDzHandler } from '$lib/dragging/stackingReorderDropzoneManager';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { codegenPath } from '$lib/routes/routes.svelte';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/core/context';
	import { ReviewBadge, Icon, Tooltip, TestId, Button, Badge } from '@gitbutler/ui';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { DropzoneHandler } from '$lib/dragging/handler';
	import type { RuleFilter } from '$lib/rules/rule';
	import type { PushStatus } from '$lib/stacks/stack';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	interface BranchCardProps {
		type: 'draft-branch' | 'normal-branch' | 'stack-branch' | 'pr-branch';
		projectId: string;
		branchName: string;
		isCommitting?: boolean;
		lineColor: string;
		readonly: boolean;
		first?: boolean;
	}

	interface DraftBranchProps extends BranchCardProps {
		type: 'draft-branch';
		branchContent: Snippet;
		buttons?: Snippet;
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
		reviewId?: string;
		pushStatus: PushStatus;
		lastUpdatedAt?: number;
		isConflicted: boolean;
		contextMenu?: typeof BranchHeaderContextMenu;
		dropzones: DropzoneHandler[];
		onclick: () => void;
		menu?: Snippet<[{ rightClickTrigger: HTMLElement }]>;
		buttons?: Snippet;
		branchContent: Snippet;
	}

	interface PrBranchProps extends BranchCardProps {
		type: 'pr-branch';
		selected: boolean;
		trackingBranch: string;
		lastUpdatedAt: number;
	}

	type Props = DraftBranchProps | NormalBranchProps | StackBranchProps | PrBranchProps;

	let { projectId, branchName, lineColor, readonly, ...args }: Props = $props();

	const uiState = inject(UI_STATE);
	const stackService = inject(STACK_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	const rulesService = inject(RULES_SERVICE);
	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);

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
	const isPushed = $derived(!!(args.type === 'draft-branch' ? undefined : args.trackingBranch));

	async function updateBranchName(title: string) {
		if (args.type === 'draft-branch') {
			uiState.global.draftBranchName.set(title);
			const normalized = await stackService.normalizeBranchName(title);
			if (normalized) {
				uiState.global.draftBranchName.set(normalized);
			}
		} else if (args.type === 'stack-branch') {
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
	class:draft={args.type === 'draft-branch'}
	data-series-name={branchName}
	data-testid={TestId.BranchCard}
>
	{#if args.type === 'stack-branch'}
		{@const moveHandler = args.stackId
			? new MoveCommitDzHandler(stackService, args.stackId, projectId)
			: undefined}
		{#if !args.prNumber && args.stackId}
			<PrNumberUpdater {projectId} stackId={args.stackId} {branchName} />
		{/if}
		{@const rule = args.stackId
			? rulesService.aiRuleForStack({ projectId, stackId: args.stackId })
			: undefined}
		{@const codegenRuleHandler = args.stackId
			? new CodegenRuleDropHandler(projectId, args.stackId, rulesService)
			: undefined}

		<Dropzone
			handlers={args.first
				? [moveHandler, codegenRuleHandler, ...args.dropzones].filter(isDefined)
				: args.dropzones}
		>
			{#snippet overlay({ hovered, activated, handler })}
				{@const label =
					handler instanceof MoveCommitDzHandler
						? 'Move here'
						: handler instanceof ReorderCommitDzHandler
							? 'Reorder here'
							: handler instanceof CodegenRuleDropHandler
								? 'Move here'
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
				iconName={args.iconName}
				{updateBranchName}
				isUpdatingName={nameUpdate.current.isLoading}
				failedMisserablyToUpdateBranchName={nameUpdate.current.isError}
				{readonly}
				{isPushed}
				onclick={args.onclick}
				menu={args.menu}
			>
				{#snippet buttons()}
					{#if args.buttons}
						{@render args.buttons()}
					{/if}
					{#if args.first && rule}
						<ReduxResult result={rule?.current} {projectId} stackId={args.stackId}>
							{#snippet children({ rule }, { projectId: _projectId, stackId: _stackId })}
								{#if rule}
									<ClaudeSessionDescriptor
										{projectId}
										sessionId={(rule.filters[0]! as RuleFilter & { type: 'claudeCodeSessionId' })
											.subject}
									>
										{#snippet loading()}
											<Button
												icon="ai-small"
												style="purple"
												size="tag"
												class="branch-header__ai-pill__name"
												shrinkable
												reversedDirection
												disabled>Loading...</Button
											>
										{/snippet}
										{#snippet error()}
											<Badge size="tag" style="error" kind="solid" icon="ai-small">
												Session error
											</Badge>
										{/snippet}
										{#snippet children(descriptor)}
											<div
												class="branch-header__ai-pill"
												use:draggableChips={{
													label: descriptor,
													data: new CodegenRuleDropData(rule),
													chipType: 'ai-session',
													dropzoneRegistry,
													dragStateService
												}}
											>
												<Button
													icon="ai-small"
													style="purple"
													size="tag"
													shrinkable
													tooltip="Click to go to session or drag to another lane"
													tooltipMaxWidth={160}
													width="100%"
													maxWidth={140}
													onclick={() => {
														if (!args.stackId) return;
														projectState.selectedClaudeSession.set({
															stackId: args.stackId,
															head: branchName
														});
														goto(codegenPath(projectId));
													}}
												>
													{#snippet custom()}
														<div class="branch-header__ai-pill-label">
															<span class="truncate">{descriptor}</span>
															<Icon name="draggable" opacity={0.6} />
														</div>
													{/snippet}
												</Button>
											</div>
										{/snippet}
									</ClaudeSessionDescriptor>
								{/if}
							{/snippet}
						</ReduxResult>
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

					{#if args.isConflicted}
						<Tooltip text="This branch has conflicts">
							<Icon name="warning-small" color="error" />
						</Tooltip>
					{/if}

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
								{@const prResult = prService?.get(args.prNumber, { forceRefetch: true })}
								{@const pr = prResult?.current.data}
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
			</BranchHeader>
		</Dropzone>
		{#if showPrCreation}
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
		{/if}
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
	{:else}
		<BranchHeader
			{branchName}
			isEmpty
			selected
			draft
			{lineColor}
			iconName="branch-local"
			{updateBranchName}
			isUpdatingName={nameUpdate.current.isLoading}
			failedMisserablyToUpdateBranchName={nameUpdate.current.isError}
			readonly={false}
			isPushed={false}
		>
			{#snippet emptyState()}
				A new branch will be created for your commit.
				<br />
				Click the name to rename it now or later.
			{/snippet}
		</BranchHeader>
	{/if}

	{#if args.type === 'stack-branch' || args.type === 'normal-branch' || args.type === 'draft-branch'}
		{@render args.branchContent()}
	{/if}
</div>

<style lang="postcss">
	.branch-card {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
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

		&:not(.no-padding) {
			padding: 12px;
		}
	}

	.branch-header__ai-pill {
		display: flex;
		overflow: hidden;
	}

	.branch-header__ai-pill-label {
		display: flex;
		align-items: center;
		padding-left: 4px;
		overflow: hidden;
		gap: 3px;
	}
</style>
