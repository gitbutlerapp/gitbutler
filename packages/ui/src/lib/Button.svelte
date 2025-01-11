<script lang="ts" module>
	export interface Props {
		id?: string | undefined;
		el?: HTMLElement;
		// Interaction props
		disabled?: boolean;
		loading?: boolean;
		activated?: boolean;
		tabindex?: number | undefined;
		type?: 'submit' | 'reset' | 'button' | undefined;
		// Layout props
		shrinkable?: boolean;
		reversedDirection?: boolean;
		width?: number | undefined;
		maxWidth?: number | undefined;
		size?: 'tag' | 'button' | 'cta';
		wide?: boolean;
		grow?: boolean;
		align?: 'flex-start' | 'center' | 'flex-end' | 'stretch' | 'baseline' | 'auto';
		dropdownChild?: boolean;
		// Style props
		style?: ComponentColorType;
		kind?: ComponentKindType;
		outline?: boolean;
		dashed?: boolean;
		solidBackground?: boolean;
		borderRadius?: 'medium' | 'large';
		// Additional elements
		icon?: keyof typeof iconsJson | undefined;
		tooltip?: string;
		tooltipPosition?: TooltipPosition;
		tooltipAlign?: TooltipAlign;
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
	import Tooltip, { type TooltipAlign, type TooltipPosition } from './Tooltip.svelte';
	import Icon from '$lib/Icon.svelte';
	import { pxToRem } from '$lib/utils/pxToRem';
	import type iconsJson from '$lib/data/icons.json';
	import type { ComponentColorType, ComponentKindType } from '$lib/utils/colorTypes';
	import type { Snippet } from 'svelte';

	let {
		el = $bindable(),
		disabled = false,
		id = undefined,
		loading = false,
		activated = false,
		tabindex,
		type = 'button',
		shrinkable = false,
		reversedDirection = false,
		width,
		maxWidth,
		size = 'button',
		wide = false,
		grow = false,
		align = 'auto',
		dropdownChild = false,
		style = 'neutral',
		kind = 'solid',
		outline = false,
		dashed = false,
		solidBackground = false,
		borderRadius = 'medium',
		testId,
		icon,
		tooltip,
		tooltipPosition,
		tooltipAlign,
		onclick,
		onmousedown,
		oncontextmenu,
		onkeydown,
		children
	}: Props = $props();

	function handleAction(e: MouseEvent) {
		if (loading || disabled) {
			e.preventDefault();
			e.stopPropagation();
		} else {
			onclick?.(e);
		}
	}
</script>

<Tooltip text={tooltip} align={tooltipAlign} position={tooltipPosition}>
	<button
		bind:this={el}
		class="btn focus-state {style} {kind} {size}-size br-{borderRadius}"
		class:outline
		class:dashed
		class:solidBackground
		class:reversed-direction={reversedDirection}
		class:shrinkable
		class:wide
		class:grow
		class:is-dropdown={dropdownChild}
		class:fixed-width={!children && !wide}
		class:activated
		style:align-self={align}
		style:width={width !== undefined ? pxToRem(width) : undefined}
		style:max-width={maxWidth !== undefined ? pxToRem(maxWidth) : undefined}
		disabled={disabled || loading}
		onclick={handleAction}
		{onmousedown}
		{oncontextmenu}
		{onkeydown}
		{type}
		{id}
		{...testId ? { 'data-testid': testId } : null}
		{tabindex}
	>
		{#if children}
			<span
				class="label text-semibold"
				class:text-12={size === 'button' || size === 'cta'}
				class:text-11={size === 'tag'}
			>
				{@render children()}
			</span>
		{/if}

		{#if icon || loading}
			<div class="btn-icon">
				{#if loading}
					<Icon name="spinner" spinnerRadius={size === 'tag' ? 4 : 5} />
				{:else if icon}
					<Icon name={icon} />
				{/if}
			</div>
		{/if}
	</button>
</Tooltip>

<style lang="postcss">
	.btn {
		user-select: none;
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		border: 1px solid var(--btn-border-clr, transparent);
		cursor: pointer;
		color: var(--btn-text-clr);
		background: var(--btn-bg, transparent);
		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast),
			max-width var(--transition-medium);
		-webkit-transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast),
			max-width var(--transition-medium);
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
		&.shrinkable {
			overflow: hidden;
			width: fit-content;

			& .label {
				display: inline-block;
				overflow: hidden;
				text-overflow: ellipsis;
			}
		}

		&.br-medium {
			border-radius: var(--radius-m);
		}

		&.br-large {
			border-radius: var(--radius-l);
		}
	}

	.label {
		pointer-events: none;
		display: inline-flex;
		white-space: nowrap;
		padding: 0 2px;
	}

	.btn-icon {
		pointer-events: none;
		flex-shrink: 0;
		display: flex;
		opacity: var(--icon-opacity);
		transition: opacity var(--transition-fast);
		/* in order to fix the transition flickering bug in Safari */
		-webkit-transform: translateZ(0);
	}

	/* STYLES */
	.ghost {
		--icon-opacity: var(--opacity-btn-icon-soft);
		--btn-text-clr: var(--clr-theme-ntrl-on-soft);

		&:not(&:disabled):hover,
		&.activated {
			--icon-opacity: var(--opacity-btn-icon-soft-hover);
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

	.neutral {
		&.soft {
			--icon-opacity: var(--opacity-btn-icon-soft);
			--btn-text-clr: var(--clr-theme-ntrl-on-soft);
			--btn-bg: var(--clr-theme-ntrl-soft);
			--btn-border-clr: var(--clr-theme-ntrl-soft-outline);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-soft-hover);
				--btn-bg: var(--clr-theme-ntrl-soft-hover);
			}
		}
		&.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--btn-text-clr: var(--clr-theme-ntrl-on-element);
			--btn-bg: var(--clr-theme-ntrl-element);

			/* if button */
			&:not(&:disabled):hover {
				--icon-opacity: var(--opacity-btn-icon-solid-hover);
				--btn-bg: var(--clr-theme-ntrl-element-hover);
			}
		}
	}

	.pop {
		&.soft {
			--icon-opacity: var(--opacity-btn-icon-soft);
			--btn-text-clr: var(--clr-theme-pop-on-soft);
			--btn-bg: var(--clr-theme-pop-soft);
			--btn-border-clr: var(--clr-theme-pop-soft-outline);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-soft-hover);
				--btn-bg: var(--clr-theme-pop-soft-hover);
			}
		}
		&.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--btn-text-clr: var(--clr-theme-pop-on-element);
			--btn-bg: var(--clr-theme-pop-element);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-solid-hover);
				--btn-bg: var(--clr-theme-pop-element-hover);
			}
		}
	}

	.success {
		&.soft {
			--icon-opacity: var(--opacity-btn-icon-soft);
			--btn-text-clr: var(--clr-theme-succ-on-soft);
			--btn-bg: var(--clr-theme-succ-soft);
			--btn-border-clr: var(--clr-theme-succ-soft-outline);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-soft-hover);
				--btn-bg: var(--clr-theme-succ-soft-hover);
			}
		}
		&.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--btn-text-clr: var(--clr-theme-succ-on-element);
			--btn-bg: var(--clr-theme-succ-element);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-solid-hover);
				--btn-bg: var(--clr-theme-succ-element-hover);
			}
		}
	}

	.error {
		&.soft {
			--icon-opacity: var(--opacity-btn-icon-soft);
			--btn-text-clr: var(--clr-theme-err-on-soft);
			--btn-bg: var(--clr-theme-err-soft);
			--btn-border-clr: var(--clr-theme-err-soft-outline);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-soft-hover);
				--btn-bg: var(--clr-theme-err-soft-hover);
			}
		}
		&.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--btn-text-clr: var(--clr-theme-err-on-element);
			--btn-bg: var(--clr-theme-err-element);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-solid-hover);
				--btn-bg: var(--clr-theme-err-element-hover);
			}
		}
	}

	.warning {
		&.soft {
			--icon-opacity: var(--opacity-btn-icon-soft);
			--btn-text-clr: var(--clr-theme-warn-on-soft);
			--btn-bg: var(--clr-theme-warn-soft);
			--btn-border-clr: var(--clr-theme-warn-soft-outline);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-soft-hover);
				--btn-bg: var(--clr-theme-warn-soft-hover);
			}
		}
		&.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--btn-text-clr: var(--clr-theme-warn-on-element);
			--btn-bg: var(--clr-theme-warn-element);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-solid-hover);
				--btn-bg: var(--clr-theme-warn-element-hover);
			}
		}
	}

	.purple {
		&.soft {
			--icon-opacity: var(--opacity-btn-icon-soft);
			--btn-text-clr: var(--clr-theme-purp-on-soft);
			--btn-bg: var(--clr-theme-purp-soft);
			--btn-border-clr: var(--clr-theme-purp-soft-outline);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-soft-hover);
				--btn-bg: var(--clr-theme-purp-soft-hover);
			}
		}
		&.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--btn-text-clr: var(--clr-theme-purp-on-element);
			--btn-bg: var(--clr-theme-purp-element);

			/* if button */
			&:not(&:disabled):hover,
			&.activated {
				--icon-opacity: var(--opacity-btn-icon-solid-hover);
				--btn-bg: var(--clr-theme-purp-element-hover);
			}
		}
	}

	/* SIZE MODIFIERS */

	.btn.tag-size {
		gap: 2px;
		height: var(--size-tag);
		padding: 2px 4px;
	}

	.btn.button-size {
		gap: 4px;
		height: var(--size-button);
		padding: 4px 6px;
	}

	.btn.cta-size {
		gap: 4px;
		height: var(--size-cta);
		padding: 6px 8px;
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

			&.solid {
				&.neutral,
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
						opacity: 0.4;
					}
				}
			}
		}

		&:last-of-type {
			border-top-left-radius: 0;
			border-bottom-left-radius: 0;
		}
	}
</style>
