<script lang="ts">
	import { Icon } from '@gitbutler/ui';
	import type { Snippet } from 'svelte';

	const {
		title,
		loading = false,
		onmousedown,
		onclick,
		icon,
		message,
		dimMessage,
		row,
		rowReverse,
		testId
	}: {
		title: string;
		loading?: boolean;
		onmousedown?: (e: MouseEvent) => void;
		onclick?: (e: MouseEvent) => void;
		icon: Snippet;
		message: Snippet;
		dimMessage?: boolean;
		row?: boolean;
		rowReverse?: boolean;
		testId?: string;
	} = $props();
</script>

<button
	type="button"
	class="action__wrapper"
	class:loading
	class:row
	class:row-reverse={rowReverse}
	{onclick}
	{onmousedown}
	disabled={loading}
	data-testid={testId}
>
	<div class="icon">
		{@render icon()}
	</div>
	<div class="action__content">
		<div class="action__title text-18 text-bold">{title}</div>
		<div class="action__message text-12 text-body" class:dim-message={dimMessage}>
			{@render message()}
		</div>
	</div>
	{#if loading}
		<div class="action__spinner">
			<Icon name="spinner" />
		</div>
	{/if}
</button>

<style lang="postcss">
	.action__wrapper {
		display: flex;
		position: relative;
		position: relative;
		flex-direction: column;
		width: 100%;
		height: auto;
		padding: 16px;
		overflow: hidden;
		gap: 20px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-1);

		text-align: left;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:hover,
		&:focus {
			outline: none;
			background-color: var(--clr-bg-1-muted);
		}
	}

	.action__wrapper.row {
		flex-direction: row;
	}

	.action__wrapper.row-reverse {
		flex-direction: row-reverse;
	}

	.loading {
		background-color: var(--clr-bg-2);
		opacity: 0.6;
		pointer-events: none;
	}

	.action__content {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 8px;
		transition: opacity var(--transition-slow);
	}

	.action__spinner {
		display: flex;
		position: absolute;
		top: 10px;
		right: 10px;
	}

	.action__title {
		color: var(--clr-text-1);
	}

	.action__message {
		max-width: 90%;
		color: var(--clr-text-2);
	}

	.dim-message {
		color: var(--clr-text-3);
	}

	.icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
	}
</style>
