<script lang="ts" module>
	export type DragableBranchData = {
		disabled: boolean;
		label: string;
		pushStatus: PushStatus | undefined;
		data: BranchDropData | undefined;
	};
</script>

<script lang="ts">
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import BranchLabel from '$components/BranchLabel.svelte';
	import CommitGoesHere from '$components/CommitGoesHere.svelte';
	import { BranchDropData } from '$lib/branches/dropHandler';
	import { draggableBranch, type DraggableConfig } from '$lib/dragging/draggable';
	import { DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import { inject } from '@gitbutler/core/context';
	import { Badge, TestId, Icon } from '@gitbutler/ui';
	import { DRAG_STATE_SERVICE } from '@gitbutler/ui/drag/dragStateService.svelte';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { slide } from 'svelte/transition';
	import type { PushStatus } from '$lib/stacks/stack';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;
		isEmpty: boolean | undefined;
		selected: boolean;
		readonly: boolean;
		draft: boolean;
		isCommitting?: boolean;
		isCommitTarget?: boolean;
		commitId?: string;
		onCommitGoesHereClick?: () => void;
		isPushed: boolean;
		lineColor: string;
		conflicts?: boolean;
		iconName: keyof typeof iconsJson;
		roundedBottom?: boolean;
		onclick?: () => void;
		disableClick?: boolean;
		updateBranchName: (name: string) => void;
		isUpdatingName: boolean;
		failedMisserablyToUpdateBranchName: boolean;
		emptyState?: Snippet;
		content?: Snippet;
		menu?: Snippet<[{ rightClickTrigger: HTMLElement }]>;
		buttons?: Snippet;
		prCreation?: Snippet;
		changedFiles?: Snippet;
		showPrCreation?: boolean;
		dragArgs?: DragableBranchData;
	};

	const {
		branchName,
		isEmpty = false,
		selected,
		draft,
		isCommitting,
		isCommitTarget = false,
		commitId,
		onCommitGoesHereClick,
		isUpdatingName,
		failedMisserablyToUpdateBranchName,
		readonly,
		isPushed,
		lineColor,
		conflicts,
		iconName,
		roundedBottom,
		onclick,
		disableClick,
		updateBranchName,
		emptyState,
		content,
		menu,
		buttons,
		prCreation,
		changedFiles,
		showPrCreation,
		dragArgs
	}: Props = $props();

	const dropzoneRegistry = inject(DROPZONE_REGISTRY);
	const dragStateService = inject(DRAG_STATE_SERVICE);

	let rightClickTrigger = $state<HTMLDivElement>();
	let active = $state(false);

	const actionsVisible = $derived(!draft && !isCommitting && (buttons || menu));
	// Show CommitGoesHere in header for:
	// 1. Draft branches (always when committing)
	// 2. Empty branches with click handler (so you can select them)
	// Branches with commits show it between commits in BranchCommitList instead
	const showCommitGoesHere = $derived(
		isCommitting && (draft || (isEmpty && onCommitGoesHereClick))
	);

	const draggableBranchConfig = $derived.by<DraggableConfig>(() => {
		if (!dragArgs) {
			return {
				disabled: true,
				dropzoneRegistry,
				dragStateService
			};
		}
		return {
			...dragArgs,
			dropzoneRegistry,
			dragStateService
		};
	});
</script>

<div
	class="header-wrapper"
	class:rounded={roundedBottom}
	use:focusable={{
		onAction: () => onclick?.(),
		onActive: (value) => (active = value),
		focusable: true
	}}
>
	<div
		data-testid={TestId.BranchHeader}
		data-testid-branch-header={branchName}
		bind:this={rightClickTrigger}
		role="button"
		class="branch-header"
		class:selected
		class:active
		class:committing={isCommitting}
		class:disable-hover={disableClick}
		{onclick}
		onkeypress={onclick}
		tabindex="0"
		data-remove-from-panning
		use:draggableBranch={draggableBranchConfig}
	>
		{#if dragArgs && !dragArgs.disabled && !conflicts}
			<div class="branch-header__drag-handle" data-no-drag>
				<Icon name="draggable-narrow" />
			</div>
		{/if}

		<div class="branch-header__content">
			{#if selected && !draft}
				<div
					class="branch-header__select-indicator"
					in:slide={{ axis: 'x', duration: 150 }}
					class:active
				></div>
			{/if}

			<div class="branch-header__title text-14 text-bold">
				<div class="branch-header__title-content">
					<BranchHeaderIcon color={lineColor} {iconName} />
					<BranchLabel
						name={branchName}
						fontSize="15"
						disabled={isUpdatingName}
						error={failedMisserablyToUpdateBranchName}
						readonly={readonly || isPushed}
						onChange={(name) => updateBranchName(name)}
					/>
				</div>

				{#if conflicts}
					<div class="branch-header__top-badges">
						<Badge style="danger">Conflicts</Badge>
					</div>
				{/if}
			</div>

			{#if isEmpty}
				<p class="text-12 text-body branch-header__empty-state">
					{@render emptyState?.()}
				</p>
			{:else if content}
				<div class="text-12 branch-header__details">
					{@render content()}
				</div>
			{/if}
		</div>
	</div>

	{#if changedFiles && selected}
		<div class="changed-files-container">
			{@render changedFiles()}
		</div>
	{/if}

	{#if showCommitGoesHere}
		<CommitGoesHere
			{commitId}
			selected={isCommitTarget}
			draft={draft || isEmpty}
			last={isEmpty && !draft}
			onclick={onCommitGoesHereClick}
		/>
	{/if}

	{#if actionsVisible && !showPrCreation}
		<div class="branch-hedaer__actions-row" class:draft class:new-branch={isEmpty} data-no-drag>
			{#if buttons}
				<div class="text-12 branch-header__actions">
					{@render buttons()}
				</div>
			{/if}

			{#if menu}
				<div class="branch-header__menu">
					{@render menu({ rightClickTrigger })}
				</div>
			{/if}
		</div>
	{/if}

	{#if prCreation && showPrCreation}
		{@render prCreation()}
	{/if}
</div>

<style lang="postcss">
	.header-wrapper {
		display: flex;
		flex-shrink: 0;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-bottom: none;
		border-radius: var(--radius-ml) var(--radius-ml) 0 0;

		&.rounded {
			border-bottom: 1px solid var(--clr-border-2);
			border-radius: var(--radius-ml);
		}
	}

	.branch-header {
		--branch-selected-bg: var(--clr-bg-1);
		--branch-selected-element-bg: var(--clr-selected-not-in-focus-element);
		--branch-side-padding: 12px;
		display: flex;
		position: relative;
		flex-direction: column;
		align-items: center;
		justify-content: flex-start;
		padding-right: 10px;
		padding-left: var(--branch-side-padding);
		overflow: hidden;
		border-bottom: none;
		background-color: var(--branch-selected-bg);
		cursor: pointer;

		&.disable-hover {
			cursor: default;
		}

		/* Selected but NOT in focus */
		&:not(.disable-hover):hover {
			--branch-selected-bg: var(--hover-bg-1);

			& .branch-header__drag-handle {
				width: 16px;
				opacity: 0.4;
			}
		}

		/* &:not(:focus-within).selected {
			--branch-selected-bg: var(--clr-selected-not-in-focus-bg);
		} */

		/* Selected in focus */
		&.active.selected {
			/* --branch-selected-bg: var(--clr-selected-in-focus-bg); */
			--branch-selected-element-bg: var(--clr-selected-in-focus-element);
		}
	}

	.branch-header__details {
		display: flex;
		align-items: center;
		padding-right: var(--branch-side-padding);
		overflow: hidden;
		gap: 6px;
		color: var(--clr-text-2);

		&:empty {
			display: none;
		}
	}

	.branch-header__title {
		display: flex;
		flex-grow: 1;
		align-items: center;
		justify-content: space-between;
		min-width: 0;
	}

	.branch-header__title-content {
		display: flex;
		flex-grow: 1;
		align-items: center;
		min-width: 0;
		padding-right: 8px;
		gap: 6px;
	}

	.branch-header__top-badges {
		display: flex;
		align-items: center;
		gap: 4px;
		transform: translateY(-2px);
	}

	.branch-header__content {
		display: flex;
		position: relative;
		flex: 1;
		flex-direction: column;
		width: 100%;
		padding: 14px 0;
		gap: 8px;
	}

	.branch-header__select-indicator {
		position: absolute;
		top: 14px;
		left: calc(var(--branch-side-padding) * -1);
		width: 4px;
		height: calc(100% - 28px);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--branch-selected-element-bg);
		transition: transform var(--transition-fast);

		&.active {
			background-color: var(--clr-selected-in-focus-element);
		}
	}

	.branch-header__drag-handle {
		display: flex;
		position: absolute;
		top: 6px;
		right: 4px;
		align-items: center;
		justify-content: flex-end;
		width: 10px;
		color: var(--clr-text-1);
		opacity: 0;
		transition:
			width var(--transition-fast),
			opacity var(--transition-fast);
	}

	.changed-files-container {
		display: flex;
		z-index: 1;
		width: 100%;
		margin-top: -6px;
		padding-top: 6px;
		padding-bottom: 14px;
		overflow: hidden;
		background-color: var(--clr-bg-1);
	}

	.branch-header__empty-state {
		padding-right: var(--branch-side-padding);
		color: var(--clr-text-2);
		opacity: 0.8;
	}

	.branch-hedaer__actions-row {
		display: flex;
		padding: 10px;
		gap: 10px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);

		/* MODIFIERS */
		&.new-branch {
			border-bottom: none;
		}
	}

	.branch-header__actions {
		display: flex;
		flex: 1;
		width: 100%;
		overflow: hidden;
		gap: 6px;
	}

	.branch-header__menu {
		display: flex;
	}
</style>
