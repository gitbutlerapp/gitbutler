<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { Action } from 'svelte/action';

	interface Props {
		orientation?: 'row' | 'column';
		centerAlign?: boolean;
		extraPadding?: boolean;
		roundedTop?: boolean;
		roundedBottom?: boolean;
		topDivider?: boolean;
		bottomBorder?: boolean;
		background?: 'loading' | 'success' | 'error' | undefined;
		noBorder?: boolean;
		labelFor?: string;
		disabled?: boolean;
		clickable?: boolean;
		iconSide?: Snippet;
		title?: Snippet;
		caption?: Snippet;
		children?: Snippet;
		actions?: Snippet;
		onclick?: (e: MouseEvent) => void;
		focusable?: Action<HTMLElement, object>;
	}

	let {
		orientation = 'column',
		centerAlign = false,
		extraPadding = false,
		roundedTop = true,
		roundedBottom = true,
		topDivider = false,
		bottomBorder = true,
		background = undefined,
		noBorder = false,
		labelFor = '',
		disabled = false,
		clickable = false,
		iconSide,
		title,
		caption,
		children,
		actions,
		focusable,
		onclick
	}: Props = $props();
	const focusableWithFallback = focusable || (() => {});
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<label
	for={labelFor}
	class="section-card"
	style:flex-direction={orientation}
	class:center-align={centerAlign && orientation === 'row'}
	class:extra-padding={extraPadding}
	class:rounded-top={roundedTop}
	class:rounded-bottom={roundedBottom}
	class:top-divider={topDivider}
	class:bottom-border={bottomBorder}
	class:no-border={noBorder}
	class:loading={background === 'loading'}
	class:success={background === 'success'}
	class:error={background === 'error'}
	class:clickable={labelFor !== '' || clickable}
	class:disabled
	use:focusableWithFallback={{}}
	{onclick}
>
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

	{@render children?.()}

	{#if actions}
		<div class="section-card__actions">
			{@render actions?.()}
		</div>
	{/if}
</label>

<style lang="postcss">
	.section-card {
		display: flex;
		position: relative;
		padding: 16px;
		gap: 16px;
		border-right-width: 1px;
		border-left-width: 1px;
		border-color: var(--clr-border-2);
		background-color: var(--clr-bg-1);
		text-align: left;
		cursor: default;
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);
	}

	.loading {
		background: var(--clr-bg-2);
	}

	.success {
		background: var(--clr-theme-pop-bg-muted);
	}

	.error {
		background: var(--clr-theme-warn-bg-muted);
	}
	.extra-padding {
		padding: 20px;
	}

	.section-card__content {
		display: flex;
		flex: 1;
		flex-direction: column;
		gap: 6px;
		user-select: text;
	}

	.section-card__title {
		color: var(--clr-scale-ntrl-0);
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

	.rounded-top {
		border-top-width: 1px;
		border-top-right-radius: var(--radius-m);
		border-top-left-radius: var(--radius-m);
	}

	.rounded-bottom {
		border-bottom-right-radius: var(--radius-m);
		border-bottom-left-radius: var(--radius-m);
	}

	.top-divider {
		&::before {
			display: block;
			position: absolute;
			top: 0;
			left: 0;
			width: 100%;
			height: 1px;
			background-color: var(--clr-border-3);
			content: '';
		}
	}

	.bottom-border {
		border-bottom-width: 1px;
	}

	.no-border {
		border-width: none;
	}

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
