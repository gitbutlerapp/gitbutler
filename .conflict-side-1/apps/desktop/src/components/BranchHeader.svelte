<!-- BRANCH HEADER PURE COMPONENT -->
<!-- ONLY CONCERNED ABOUT STYLE 😎 -->
<script lang="ts">
	import BranchHeaderIcon from '$components/BranchHeaderIcon.svelte';
	import BranchLabel from '$components/BranchLabel.svelte';
	import { TestId } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;
		isEmpty: boolean | undefined;
		selected: boolean;
		active: boolean | undefined;
		readonly: boolean;
		draft: boolean;
		isCommitting?: boolean;
		isPushed: boolean;
		lineColor: string;
		iconName: keyof typeof iconsJson;
		onclick?: () => void;
		updateBranchName: (name: string) => void;
		isUpdatingName: boolean;
		failedMisserablyToUpdateBranchName: boolean;
		emptyState?: Snippet;
		content?: Snippet;
		menu?: Snippet<[{ rightClickTrigger: HTMLElement }]>;
		buttons?: Snippet;
	};

	const {
		branchName,
		isEmpty = false,
		selected,
		draft,
		active,
		isCommitting,
		isUpdatingName,
		failedMisserablyToUpdateBranchName,
		readonly,
		isPushed,
		lineColor,
		iconName,
		onclick,
		updateBranchName,
		emptyState,
		content,
		menu,
		buttons
	}: Props = $props();

	let rightClickTrigger = $state<HTMLDivElement>();

	const actionsVisible = $derived(!draft && !isCommitting && (buttons || menu));
</script>

<div
	data-testid={TestId.BranchHeader}
	data-testid-branch-header={branchName}
	bind:this={rightClickTrigger}
	role="button"
	class="branch-header"
	class:selected
	class:active
	class:draft
	class:commiting={isCommitting}
	{onclick}
	onkeypress={onclick}
	tabindex="0"
>
	{#if selected && !draft}
		<div
			class="branch-header__select-indicator"
			in:slide={{ axis: 'x', duration: 150 }}
			class:active
		></div>
	{/if}

	<div class="branch-header__content">
		<div class="branch-header__title text-14 text-bold">
			<div class="branch-header__title-content flex gap-6">
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

{#if actionsVisible}
	<div class="branch-hedaer__actions-row" class:draft class:new-branch={isEmpty}>
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

<style lang="postcss">
	.branch-header {
		--branch-selected-bg: var(--clr-bg-1);
		--branch-selected-element-bg: var(--clr-selected-not-in-focus-element);
		display: flex;

		position: relative;
		flex-direction: column;
		align-items: center;
		justify-content: flex-start;
		padding-right: 10px;
		padding-left: 15px;
		overflow: hidden;
		background-color: var(--branch-selected-bg);

		/* Selected but NOT in focus */
		&:hover {
			--branch-selected-bg: var(--clr-bg-1-muted);
		}

		&:focus-within,
		&:not(:focus-within).selected {
			--branch-selected-bg: var(--clr-selected-not-in-focus-bg);
		}

		/* Selected in focus */
		&.active.selected {
			--branch-selected-bg: var(--clr-selected-in-focus-bg);
			--branch-selected-element-bg: var(--clr-selected-in-focus-element);
		}
	}

	.branch-header__details {
		display: flex;
		align-items: center;
		overflow: hidden;
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
		flex-grow: 1;
		align-items: center;
		justify-content: space-between;
		min-width: 0;
		overflow: hidden;
		gap: 4px;
	}

	.branch-header__title-content {
		flex-grow: 1;
		align-items: center;
		min-width: 0;
	}

	.branch-header__content {
		display: flex;
		flex: 1;
		flex-direction: column;
		width: 100%;
		margin-left: -2px;
		padding: 14px 0;
		overflow: hidden;
		gap: 8px;
		text-overflow: ellipsis;
	}

	.branch-header__empty-state {
		color: var(--clr-text-2);
		opacity: 0.8;
	}

	.branch-hedaer__actions-row {
		display: flex;
		padding: 10px;
		gap: 10px;
		border-top: 1px solid var(--clr-border-2);
		/* border-bottom: 1px solid var(--clr-border-2); */
		background-color: var(--clr-bg-2);

		/* MODIFIERS */
		&.new-branch {
			border-bottom: none;
			border-radius: 0 0 var(--radius-ml) var(--radius-ml);
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
