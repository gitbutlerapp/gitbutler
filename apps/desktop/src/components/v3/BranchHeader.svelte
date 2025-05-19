<!-- BRANCH HEADER PURE COMPONENT -->
<!-- ONLY CONCERNED ABOUT STYLE 😎 -->
<script lang="ts">
	import BranchLabel from '$components/BranchLabel.svelte';
	import BranchHeaderIcon from '$components/v3/BranchHeaderIcon.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { slide } from 'svelte/transition';
	import type iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Snippet } from 'svelte';

	type Props = {
		branchName: string;
		isEmpty: boolean | undefined;
		selected: boolean;
		selectIndicator: boolean;
		active: boolean | undefined;
		readonly: boolean;
		draft: boolean;

		isPushed: boolean;

		lineColor: string;
		iconName: keyof typeof iconsJson;

		onclick?: () => void;
		updateBranchName: (name: string) => void;
		isUpdatingName: boolean;

		emptyState: Snippet;
		content?: Snippet;
		menu?: Snippet<[{ rightClickTrigger: HTMLElement }]>;
	};

	const {
		branchName,
		isEmpty = false,
		selected,
		selectIndicator,
		draft,
		active,
		isUpdatingName,
		readonly,
		isPushed,
		lineColor,
		iconName,
		onclick,
		updateBranchName,
		emptyState,
		content,
		menu
	}: Props = $props();

	let rightClickTrigger = $state<HTMLDivElement>();
</script>

<div
	data-testid={TestId.BranchHeader}
	data-testid-branch-header={branchName}
	bind:this={rightClickTrigger}
	role="button"
	class="branch-header"
	class:new-branch={isEmpty}
	class:selected
	class:draft
	{onclick}
	onkeypress={onclick}
	tabindex="0"
	class:active
>
	{#if selected && selectIndicator}
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
					readonly={readonly || isPushed}
					onChange={(name) => updateBranchName(name)}
				/>
			</div>

			{#if menu}
				<div class="branch-header__menu">
					{@render menu({ rightClickTrigger })}
				</div>
			{/if}
		</div>

		{#if isEmpty}
			<p class="text-12 text-body branch-header__empty-state">
				{@render emptyState()}
			</p>
		{:else if content}
			<div class="text-12 branch-header__details">
				{@render content()}
			</div>
		{/if}
	</div>
</div>

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
		overflow: auto;
	}

	.branch-header__title-content {
		align-items: center;
		flex-grow: 1;
		min-width: 0;
	}

	.branch-header__menu {
		display: flex;
		align-items: center;
		justify-content: center;
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
	}
</style>
