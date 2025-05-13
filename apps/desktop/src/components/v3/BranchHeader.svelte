<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import CardOverlay from '$components/CardOverlay.svelte';
	import Dropzone from '$components/Dropzone.svelte';
	import BranchBadge from '$components/v3/BranchBadge.svelte';
	import BranchHeaderContextMenu from '$components/v3/BranchHeaderContextMenu.svelte';
	import BranchHeaderIcon from '$components/v3/BranchHeaderIcon.svelte';
	import { MoveCommitDzHandler, StartCommitDzHandler } from '$lib/commits/dropHandler';
	import { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { inject } from '@gitbutler/shared/context';
	import ReviewBadge from '@gitbutler/ui/ReviewBadge.svelte';
	import { getTimeAgo } from '@gitbutler/ui/utils/timeAgo';
	import { slide } from 'svelte/transition';
	import type { PushStatus } from '$lib/stacks/stack';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	export type Props = {
		type: 'draft-branch' | 'normal-branch' | 'stack-branch';
		projectId: string;
		branchName: string;
		readonly: boolean;
		iconName: keyof typeof iconsJson;
		lineColor: string;
		active?: boolean;
	} & (
		| { type: 'draft-branch' }
		| {
				type: 'normal-branch';
				selected: boolean;
				trackingBranch?: string;
				lastUpdatedAt?: number;
				isTopBranch?: boolean;
				isNewBranch?: boolean;
				onclick: () => void;
		  }
		| {
				type: 'stack-branch';
				selected: boolean;
				stackId: string;
				trackingBranch?: string;
				isTopBranch: boolean;
				isNewBranch?: boolean;
				prNumber?: number;
				reviewId?: string;
				pushStatus: PushStatus;
				lastUpdatedAt?: number;
				isConflicted: boolean;
				contextMenu?: typeof BranchHeaderContextMenu;
				onclick: () => void;
				menu?: Snippet<
					[
						{
							rightClickTrigger: HTMLElement;
						}
					]
				>;
		  }
	);

	let { projectId, branchName, readonly, iconName, lineColor, active, ...args }: Props = $props();

	const [stackService, uiState, changeSelectionService] = inject(
		StackService,
		UiState,
		ChangeSelectionService
	);

	const [updateName, nameUpdate] = stackService.updateBranchName;

	const isPushed = $derived(!!(args.type === 'draft-branch' ? undefined : args.trackingBranch));

	let rightClickTrigger = $state<HTMLDivElement>();

	async function updateBranchName(title: string) {
		if (args.type === 'draft-branch') {
			uiState.global.draftBranchName.set(title);
			const normalized = await stackService.normalizeBranchName(title);
			if (normalized.data) {
				uiState.global.draftBranchName.set(normalized.data);
			}
		} else if (args.type === 'stack-branch') {
			updateName({
				projectId,
				stackId: args.stackId,
				branchName,
				newName: title
			});
		}
	}
</script>

{#if args.type === 'stack-branch'}
	{@const moveHandler = new MoveCommitDzHandler(stackService, args.stackId, projectId)}
	{@const startCommitHandler = new StartCommitDzHandler({
		uiState,
		changeSelectionService,
		stackId: args.stackId,
		projectId,
		branchName
	})}
	<Dropzone handlers={[moveHandler, startCommitHandler]}>
		{#snippet overlay({ hovered, activated, handler })}
			{@const label = handler instanceof MoveCommitDzHandler ? 'Move here' : 'Start commit'}
			<CardOverlay {hovered} {activated} {label} />
		{/snippet}
		<div
			data-testid={TestId.BranchHeader}
			data-testid-branch-header={branchName}
			bind:this={rightClickTrigger}
			role="button"
			class="branch-header"
			class:new-branch={args.isNewBranch}
			class:selected={args.selected}
			onclick={args.onclick}
			onkeypress={args.onclick}
			tabindex="0"
			class:active
		>
			{#if args.selected}
				<div
					class="branch-header__select-indicator"
					in:slide={{ axis: 'x', duration: 150 }}
					class:active
				></div>
			{/if}

			<div class="branch-header__content">
				<div class="branch-header__title text-14 text-bold">
					<div class="flex gap-6">
						<BranchHeaderIcon color={lineColor} {iconName} />
						<BranchLabel
							name={branchName}
							fontSize="15"
							disabled={nameUpdate.current.isLoading}
							readonly={readonly || isPushed}
							onChange={(name) => updateBranchName(name)}
						/>
					</div>

					{#if args.menu}
						{@render args.menu({ rightClickTrigger })}
					{/if}
				</div>

				{#if args.isNewBranch}
					<p class="text-12 text-body branch-header__empty-state">
						<span>This is an empty branch.</span> <span>Click for details.</span>
						<br />
						Create or drag & drop commits here.
					</p>
				{:else}
					<div class="text-12 branch-header__details">
						<span class="branch-header__item">
							<BranchBadge pushStatus={args.pushStatus} unstyled />
						</span>
						<span class="branch-header__divider">•</span>

						{#if args.isConflicted}
							<span class="branch-header__item branch-header__item--conflict"> Conflicts </span>
							<span class="branch-header__divider">•</span>
						{/if}

						{#if args.lastUpdatedAt}
							<span class="branch-header__item">
								{getTimeAgo(new Date(args.lastUpdatedAt))}
							</span>
						{/if}

						{#if args.reviewId || args.prNumber}
							<span class="branch-header__divider">•</span>
							<div class="branch-header__review-badges">
								{#if args.reviewId}
									<ReviewBadge brId={args.reviewId} brStatus="unknown" />
								{/if}
								{#if args.prNumber}
									<ReviewBadge prNumber={args.prNumber} prStatus="unknown" />
								{/if}
							</div>
						{/if}
					</div>
				{/if}
			</div>
		</div>
	</Dropzone>
{:else if args.type === 'normal-branch'}
	<div
		data-testid={TestId.BranchHeader}
		bind:this={rightClickTrigger}
		role="button"
		class="branch-header"
		class:selected={args.selected}
		class:new-branch={args.isNewBranch}
		onclick={args.onclick}
		onkeypress={args.onclick}
		tabindex="0"
	>
		{#if args.selected}
			<div
				class="branch-header__select-indicator"
				in:slide={{ axis: 'x', duration: 150 }}
				class:active
			></div>
		{/if}

		<div class="branch-header__content">
			<div class="branch-header__title text-14 text-bold">
				<div class="flex gap-6">
					<BranchHeaderIcon color={lineColor} {iconName} />
					<BranchLabel name={branchName} fontSize="15" readonly={true} />
				</div>
			</div>

			{#if args.isNewBranch}
				<p class="text-12 text-body branch-header__empty-state">
					<span>There are no commits yet on this branch.</span>
				</p>
			{:else}
				<div class="text-12 branch-header__details">
					{#if args.lastUpdatedAt}
						<span class="branch-header__item">
							{getTimeAgo(new Date(args.lastUpdatedAt))}
						</span>
					{/if}
				</div>
			{/if}
		</div>
	</div>
{:else}
	<div
		data-testid={TestId.BranchHeader}
		bind:this={rightClickTrigger}
		role="button"
		class="branch-header new-branch draft selected"
		tabindex="0"
	>
		<div class="branch-header__content">
			<div class="branch-header__title text-14 text-bold">
				<div class="flex gap-6">
					<BranchHeaderIcon color={lineColor} {iconName} />
					<BranchLabel
						allowClear
						name={branchName}
						fontSize="15"
						onChange={(name) => updateBranchName(name)}
					/>
				</div>
			</div>
			<p class="text-12 text-body branch-header__empty-state">
				A new branch will be created for your commit.
				<br />
				Click the name to rename it now or later.
			</p>
		</div>
	</div>
{/if}

<style lang="postcss">
	.branch-header {
		--branch-selected-bg: var(--clr-bg-1);
		--branch-selected-element-bg: var(--clr-selected-not-in-focus-element);

		position: relative;
		display: flex;
		justify-content: flex-start;
		align-items: center;
		padding-left: 15px;
		padding-right: 10px;
		border-top-right-radius: var(--radius-ml);
		border-top-left-radius: var(--radius-ml);
		border-bottom: 1px solid var(--clr-border-2);
		overflow: hidden;
		background-color: var(--branch-selected-bg);

		/* Selected but NOT in focus */
		&:hover {
			--branch-selected-bg: var(--clr-bg-1-muted);
		}
		&:focus-within,
		&.selected {
			--branch-selected-bg: var(--clr-selected-not-in-focus-bg);
		}

		/* Selected in focus */
		&:focus-within.selected {
			--branch-selected-bg: var(--clr-selected-in-focus-bg);
			--branch-selected-element-bg: var(--clr-selected-in-focus-element);
		}

		/* MODIFIERS */
		&.new-branch {
			border-bottom: none;
			border-radius: var(--radius-ml);
		}
	}

	.branch-header__details {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 6px;
		color: var(--clr-text-2);

		&:empty {
			display: none;
		}
	}

	.branch-header__select-indicator {
		position: absolute;
		top: 14px;
		left: 0;
		width: 4px;
		height: calc(100% - 28px);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--branch-selected-element-bg);
		transition: transform var(--transition-fast);
		&.active {
			background-color: var(--clr-selected-in-focus-element);
		}
	}

	.branch-header__title {
		display: flex;
		align-items: center;
		justify-content: space-between;
		min-width: 0;
		flex-grow: 1;
	}

	.branch-header__content {
		overflow: hidden;
		flex: 1;
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 14px 0;
		margin-left: -2px;
		text-overflow: ellipsis;
	}

	.branch-header__empty-state {
		opacity: 0.8;
		color: var(--clr-text-2);

		& span {
			text-wrap: nowrap;
		}
	}
	.branch-header__review-badges {
		display: flex;
		gap: 3px;
	}

	.branch-header__item {
		white-space: nowrap;
		color: var(--clr-text-2);
	}

	.branch-header__item--conflict {
		color: var(--clr-theme-err-element);
	}

	.branch-header__divider {
		color: var(--clr-text-3);
	}
</style>
