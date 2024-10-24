<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';
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
		width: 100%;
		height: auto;
		position: relative;
		display: flex;
		flex-direction: column;
		gap: 20px;
		overflow: hidden;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		position: relative;
		padding: 16px;

		text-align: left;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);
		background-color: var(--clr-bg-1);

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
		pointer-events: none;
		background-color: var(--clr-bg-2);
		opacity: 0.6;
	}

	.action__content {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 8px;
		transition: opacity var(--transition-slow);
	}

	.action__spinner {
		position: absolute;
		top: 10px;
		right: 10px;
		display: flex;
	}

	.action__title {
		color: var(--clr-scale-ntrl-0);
	}

	.action__message {
		color: var(--clr-text-2);
		max-width: 90%;
	}

	.dim-message {
		color: var(--clr-text-3);
	}

	.icon {
		display: flex;
		align-items: center;
		flex-shrink: 0;
	}
</style>
