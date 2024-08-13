<script lang="ts" context="module">
	export interface ButtonProps {
		el?: HTMLElement;
		// Interaction props
		disabled?: boolean;
		clickable?: boolean;
		id?: string | undefined;
		loading?: boolean;
		tabindex?: number | undefined;
		type?: 'submit' | 'reset' | 'button' | undefined;
		// Layout props
		shrinkable?: boolean;
		reversedDirection?: boolean;
		width?: number | undefined;
		size?: 'tag' | 'button' | 'cta';
		wide?: boolean;
		grow?: boolean;
		align?: 'flex-start' | 'center' | 'flex-end' | 'stretch' | 'baseline' | 'auto';
		isDropdownChild?: boolean;
		// Style props
		style?: ComponentColor;
		kind?: ComponentStyleKind;
		outline?: boolean;
		dashed?: boolean;
		solidBackground?: boolean;
		// Additional elements
		icon?: keyof typeof iconsJson | undefined;
		help?: string;
		helpShowDelay?: number;
		testId?: string;
		// Events
		onclick?: (e: MouseEvent) => void;
		onmousedown?: (e: MouseEvent) => void;
		oncontextmenu?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		// Snippets
		children?: Snippet;
	}
</script>

<script lang="ts">
	import Icon from '$lib/icon/Icon.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { tooltip } from '$lib/utils/tooltip';
	import type iconsJson from '$lib/icon/icons.json';
	import type { ComponentColor, ComponentStyleKind } from '$lib/utils/colorTypes';
	import type { Snippet } from 'svelte';

	let {
		el = $bindable(),
		disabled = false,
		clickable = true,
		id = undefined,
		loading = false,
		tabindex,
		type,
		shrinkable = false,
		reversedDirection = false,
		width,
		size = 'button',
		wide = false,
		grow = false,
		align = 'auto',
		isDropdownChild = false,
		style = 'neutral',
		kind = 'soft',
		outline = false,
		dashed = false,
		solidBackground = false,
		testId,
		icon,
		help = '',
		helpShowDelay = 1200,
		onclick,
		onmousedown,
		oncontextmenu,
		onkeydown,
		children
	}: ButtonProps = $props();

	function handleAction(e: MouseEvent) {
		if (loading || disabled || !clickable) {
			e.preventDefault();
			e.stopPropagation();
		} else {
			onclick?.(e);
		}
	}
</script>

<button
	bind:this={el}
	class="btn focus-state {style} {kind} {size}-size"
	class:outline
	class:dashed
	class:solidBackground
	class:reversed-direction={reversedDirection}
	class:shrinkable
	class:wide
	class:grow
	class:not-clickable={!clickable}
	class:fixed-width={!children && !wide}
	class:is-dropdown={isDropdownChild}
	style:align-self={align}
	style:width={width ? pxToRem(width) : undefined}
	use:tooltip={{
		text: help,
		delay: helpShowDelay
	}}
	disabled={disabled || loading}
	onclick={handleAction}
	{onmousedown}
	{oncontextmenu}
	{onkeydown}
	{type}
	{id}
	{...testId ? { 'data-testid': testId } : null}
	tabindex={clickable ? tabindex : -1}
>
	{#if children}
		<span
			class="label text-semibold"
			class:text-base-12={size === 'button' || size === 'cta'}
			class:text-base-11={size === 'tag'}
		>
			{@render children()}
		</span>
	{/if}

	{#if icon || loading}
		<div class="btn-icon">
			{#if loading}
				<Icon name="spinner" spinnerRadius={4.5} />
			{:else if icon}
				<Icon name={icon} />
			{/if}
		</div>
	{/if}
</button>

<style lang="postcss">
	.btn {
		user-select: none;
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-m);
		border: 1px solid transparent;
		cursor: pointer;
		color: var(--btn-text-clr);
		background: var(--btn-bg);
		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast);
		-webkit-transform-style: preserve-3d;
		-webkit-backface-visibility: hidden;

		&:disabled {
			cursor: default;
			opacity: 0.5;
		}
		&.wide {
			display: flex;
			width: 100%;
		}
		&.grow {
			flex-grow: 1;
		}
		&.reversed-direction {
			flex-direction: row-reverse;
		}
		&.not-clickable {
			cursor: default;

			&:focus-within {
				outline: none;
				animation: none;
			}
		}
		&.shrinkable {
			overflow: hidden;
			width: fit-content;

			& .label {
				display: inline-block;
				overflow: hidden;
				text-overflow: ellipsis;
			}
		}
	}

	.label {
		display: inline-flex;
		white-space: nowrap;
	}

	.btn-icon {
		flex-shrink: 0;
		display: flex;
		opacity: var(--icon-opacity);
		transition: opacity var(--transition-fast);
		/* in order to fix the transition flickering bug in Safari */
		-webkit-transform: translateZ(0);
	}

	/* STYLES */
	.neutral {
		--icon-opacity: 0.6;
		/* kind */
		&.soft {
			--btn-text-clr: var(--clr-theme-ntrl-on-soft);
			--btn-bg: var(--clr-theme-ntrl-soft);

			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.8;
				--btn-bg: var(--clr-theme-ntrl-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-ntrl-on-element);
			--btn-bg: var(--clr-theme-ntrl-element);

			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.8;
				--btn-bg: var(--clr-theme-ntrl-element-hover);
			}
		}
	}

	.ghost {
		--icon-opacity: 0.6;
		--btn-text-clr: var(--clr-theme-ntrl-on-soft);
		--btn-bg: transparent;

		&:not(.not-clickable, &:disabled):hover {
			--icon-opacity: 0.8;
			--btn-bg: var(--clr-bg-1-muted);
		}

		&.outline {
			border: 1px solid var(--clr-border-2);
		}

		&.dashed {
			border-style: dashed;
		}

		&.solidBackground {
			background: var(--clr-bg-1);
		}
	}

	.pop {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-pop-on-soft);
			--btn-bg: var(--clr-theme-pop-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-pop-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-pop-on-element);
			--btn-bg: var(--clr-theme-pop-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-pop-element-hover);
			}
		}
	}

	.success {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-succ-on-soft);
			--btn-bg: var(--clr-theme-succ-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-succ-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-succ-on-element);
			--btn-bg: var(--clr-theme-succ-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-succ-element-hover);
			}
		}
	}

	.error {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-err-on-soft);
			--btn-bg: var(--clr-theme-err-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-err-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-err-on-element);
			--btn-bg: var(--clr-theme-err-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-err-element-hover);
			}
		}
	}

	.warning {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-warn-on-soft);
			--btn-bg: var(--clr-theme-warn-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-warn-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-warn-on-element);
			--btn-bg: var(--clr-theme-warn-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-warn-element-hover);
			}
		}
	}

	.purple {
		--icon-opacity: 0.8;

		&.soft {
			--btn-text-clr: var(--clr-theme-purp-on-soft);
			--btn-bg: var(--clr-theme-purp-soft);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-purp-soft-hover);
			}
		}
		&.solid {
			--btn-text-clr: var(--clr-theme-purp-on-element);
			--btn-bg: var(--clr-theme-purp-element);
			/* if button */
			&:not(.not-clickable, &:disabled):hover {
				--icon-opacity: 0.9;
				--btn-bg: var(--clr-theme-purp-element-hover);
			}
		}
	}

	/* SIZE MODIFIERS */

	.btn.tag-size {
		height: var(--size-tag);
		padding: 2px 4px;

		& .label {
			padding: 0 4px;
		}
	}

	.btn.button-size {
		height: var(--size-button);
		padding: 4px 6px;

		& .label {
			padding: 0 4px;
		}
	}

	.btn.cta-size {
		height: var(--size-cta);
		padding: 6px 8px;

		& .label {
			padding: 0 6px;
		}
	}

	/* FIXED WIDTH */

	.btn.fixed-width {
		&.tag-size {
			width: var(--size-tag);
		}

		&.button-size {
			width: var(--size-button);
		}

		&.cta-size {
			width: var(--size-cta);
		}
	}

	/* DROPDOWN */
	.is-dropdown {
		&:first-of-type {
			flex: 1;
			border-top-right-radius: 0;
			border-bottom-right-radius: 0;
			border-right: none;

			&.pop,
			&.success,
			&.error,
			&.warning,
			&.purple {
				&:after {
					content: '';
					background-color: currentColor;
					z-index: var(--z-lifted);
					position: absolute;
					top: 0;
					right: 0;
					width: 1px;
					height: 100%;
					opacity: 0.2;
				}
			}
		}

		&:last-of-type {
			border-top-left-radius: 0;
			border-bottom-left-radius: 0;
		}
	}
</style>
