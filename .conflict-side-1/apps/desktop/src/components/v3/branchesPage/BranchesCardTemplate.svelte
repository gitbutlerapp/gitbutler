<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		content: Snippet;
		details: Snippet;
		selected?: boolean;
		onclick?: () => void;
	};

	const { content, details, selected = $bindable(), onclick }: Props = $props();
</script>

<div
	role="presentation"
	{onclick}
	class="branches-list-card"
	class:branches-list-card_selected={selected}
>
	<div class="branches-list-card__content">
		{@render content()}
	</div>

	<hr class="branches-list-card__hr" />

	<div class="text-12 branches-list-card__details">
		{@render details()}
	</div>
</div>

<style class="postcss">
	/* TARGET CARD */
	.branches-list-card {
		cursor: pointer;
		display: flex;
		flex-direction: column;
		gap: 10px;
		background-color: var(--clr-bg-1);
		padding: 14px 14px 14px 16px;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.branches-list-card__hr {
		height: 1px;
		background-color: var(--clr-text-3);
		opacity: 0.3;
		border: none;
	}

	.branches-list-card__content {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.branches-list-card__details {
		display: flex;
		gap: 6px;
		align-items: center;
		color: var(--clr-text-2);
	}

	.branches-list-card_selected {
		background-color: rgb(177, 153, 200);
	}
</style>
