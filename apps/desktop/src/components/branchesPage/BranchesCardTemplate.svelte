<script lang="ts">
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import type { Snippet } from 'svelte';

	type Props = {
		content: Snippet;
		details: Snippet;
		selected?: boolean;
		onclick?: () => void;
		testId?: string;
	};

	const { content, details, selected, onclick, testId }: Props = $props();
</script>

<div
	data-testid={testId}
	role="presentation"
	{onclick}
	class="branches-list-card"
	class:selected
	use:focusable={{
		focusable: true,
		onAction: () => onclick?.()
	}}
>
	<div class="branches-list-card__content">
		{@render content()}
	</div>

	<div class="text-12 branches-list-card__details">
		{@render details()}
	</div>
</div>

<style class="postcss">
	/* TARGET CARD */
	.branches-list-card {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 14px;
		gap: 10px;
		background-color: var(--clr-bg-1);
		cursor: pointer;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&::after {
			position: absolute;
			top: 12px;
			left: 0;
			width: 5px;
			height: calc(100% - 24px);
			transform: translateX(-100%);
			border-radius: 0 var(--radius-m) var(--radius-m) 0;
			background-color: var(--clr-selected-in-focus-element);
			content: '';
			transition: transform var(--transition-medium);
		}

		&:not(.selected):hover {
			background-color: var(--clr-bg-1-muted);
		}
	}

	.branches-list-card__content {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.branches-list-card__details {
		display: flex;
		position: relative;
		align-items: center;
		margin-top: 2px;
		padding-top: 10px;
		gap: 6px;
		color: var(--clr-text-2);

		&::before {
			position: absolute;
			top: 0;
			left: 0;
			flex-shrink: 0;
			width: 100%;
			height: 1px;
			background-color: var(--clr-text-3);
			content: '';
			opacity: 0.3;
		}

		&:empty {
			display: none;
		}
	}

	.selected {
		background-color: var(--clr-bg-1-muted);

		&::after {
			transform: translateX(0);
		}
	}
</style>
