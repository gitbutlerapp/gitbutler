<script lang="ts">
	import { focusable } from '$lib/focus/focusable';
	import type { Snippet } from 'svelte';

	interface Props {
		labelFor?: string;
		disabled?: boolean;
		background?: string;
		standalone?: boolean;
		clickable?: boolean;
		alignment?: 'top' | 'center';
		iconSide?: Snippet;
		title?: Snippet;
		caption?: Snippet;
		children?: Snippet;
		actions?: Snippet;
		onclick?: (e: MouseEvent) => void;
	}

	let {
		labelFor = '',
		disabled = false,
		clickable = false,
		alignment = 'top',
		standalone = false,
		background = 'var(--clr-bg-1)',
		iconSide,
		title,
		caption,
		children,
		actions,
		onclick
	}: Props = $props();

	const element = $derived(labelFor ? 'label' : 'div');
</script>

<svelte:element
	this={element}
	role="presentation"
	for={labelFor || undefined}
	class="section-card"
	class:clickable={labelFor !== '' || clickable}
	class:disabled
	class:standalone
	style:background
	use:focusable
	{onclick}
>
	{#if title || caption || iconSide || actions}
		<div class="flex full-width gap-16 hide-when-empty" class:center-align={alignment === 'center'}>
			{#if iconSide}
				<div class="section-card__icon-side">
					{@render iconSide?.()}
				</div>
			{/if}

			{#if title || caption}
				<div class="section-card__content">
					{#if title}
						<h3 class="text-15 text-bold section-card__title">
							{@render title?.()}
						</h3>
					{/if}
					{#if caption}
						<p class="text-12 text-body section-card__text">
							{@render caption?.()}
						</p>
					{/if}
				</div>
			{/if}

			{#if actions}
				<div class="section-card__actions">
					{@render actions?.()}
				</div>
			{/if}
		</div>
	{/if}

	{#if children}
		<div class="stack-v gap-16 hide-when-empty">
			{@render children?.()}
		</div>
	{/if}
</svelte:element>

<style lang="postcss">
	.section-card {
		display: flex;
		position: relative;
		flex-direction: column;
		padding: 16px;
		gap: 16px;
		border-bottom: 1px solid var(--clr-border-2);
		text-align: left;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:not(.standalone):last-child {
			border-bottom: none;
		}

		&.standalone {
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
		}
	}

	.disabled {
		background: var(--clr-bg-2);
	}

	.section-card__content {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 6px;
		user-select: text;
	}

	.section-card__title {
		color: var(--clr-text-1);
	}

	.section-card__text {
		color: var(--clr-text-2);

		/* if empty hide the caption */
		&:empty {
			display: none;
		}
	}

	.section-card__actions {
		display: flex;
	}

	.section-card__icon-side {
		display: flex;
	}

	/* MODIFIERS */

	.clickable {
		cursor: pointer;
	}

	.disabled {
		opacity: 0.5;
		pointer-events: none;
	}

	.center-align {
		align-items: center;
	}
</style>
