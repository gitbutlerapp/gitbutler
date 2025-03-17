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
		width?: number | string | undefined;
		maxWidth?: number | undefined;
		size?: 'icon' | 'tag' | 'button' | 'cta';
		wide?: boolean;
		grow?: boolean;
		align?: 'flex-start' | 'center' | 'flex-end' | 'stretch' | 'baseline' | 'auto';
		dropdownChild?: boolean;
		// Style props
		style?: ComponentColorType;
		kind?: ComponentKindType;
		solidBackground?: boolean;
		class?: string | (string | undefined)[] | Record<string, string>;
		iconClass?: string;
		/**
		 * Custom style to be applied to the button.
		 */
		customStyle?: string;
		// Additional elements
		icon?: keyof typeof iconsJson | undefined;
		tooltip?: string;
		tooltipPosition?: TooltipPosition;
		tooltipAlign?: TooltipAlign;
		tooltipDelay?: number;
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
		solidBackground = false,
		class: className = '',
		iconClass = '',
		customStyle,
		testId,
		icon,
		tooltip,
		tooltipPosition,
		tooltipAlign,
		tooltipDelay,
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

<Tooltip text={tooltip} align={tooltipAlign} position={tooltipPosition} delay={tooltipDelay}>
	<button
		bind:this={el}
		class={[
			'btn focus-state',
			style,
			kind,
			size && `${size}-size`,
			activated && 'activated',
			grow && 'grow',
			wide && 'wide',
			shrinkable && 'shrinkable',
			solidBackground && 'solidBackground',
			reversedDirection && 'reversed-direction',
			dropdownChild && 'is-dropdown',
			!children && !wide && 'fixed-width',
			className
		]}
		style:align-self={align}
		style:width={width !== undefined
			? typeof width === 'number'
				? pxToRem(width)
				: width
			: undefined}
		style:max-width={maxWidth !== undefined ? pxToRem(maxWidth) : undefined}
		style={customStyle}
		disabled={disabled || loading}
		onclick={handleAction}
		{onmousedown}
		{oncontextmenu}
		{onkeydown}
		{type}
		{id}
		{tabindex}
		{...testId ? { 'data-testid': testId } : null}
	>
		{#if children}
			<span
				class="label text-semibold"
				class:text-12={size === 'button' || size === 'cta'}
				class:text-11={size === 'tag'}
				class:text-10={size === 'icon'}
			>
				{@render children()}
			</span>
		{/if}

		{#if icon || loading}
			<div class={['btn-icon', iconClass]}>
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
	@layer components {
		.btn {
			user-select: none;
			position: relative;
			display: inline-flex;
			align-items: center;
			justify-content: center;
			cursor: pointer;
			border-radius: var(--radius-m);

			color: var(--label-clr);
			background: color-mix(
				in srgb,
				var(--btn-bg, transparent),
				transparent calc((1 - var(--opacity-btn-bg, 1)) * 100%)
			);

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

			&.outline,
			&.ghost {
				--opacity-btn-bg: 0;
				--icon-opacity: var(--opacity-btn-icon-outline);

				/* if button */
				&:not(&:disabled):hover,
				&.activated {
					--icon-opacity: var(--opacity-btn-icon-outline-hover);
					--opacity-btn-bg: var(--opacity-btn-outline-bg-hover);
				}
			}
			&.outline {
				--btn-border-opacity: var(--opacity-btn-outline);
				border: 1px solid
					color-mix(
						in srgb,
						var(--btn-border-clr, transparent),
						transparent calc((1 - var(--btn-border-opacity, 1)) * 100%)
					);
			}
			&:disabled {
				cursor: not-allowed;
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
		.neutral {
			&.outline,
			&.ghost {
				--label-clr: var(--clr-btn-ntrl-outline-text);
				--btn-bg: var(--clr-btn-ntrl-outline-bg);

				/* if button */
				&:not(&:disabled):hover,
				&.activated {
					--label-clr: var(--clr-btn-ntrl-outline-text-hover);
				}
			}
			&.outline {
				--btn-border-clr: var(--clr-btn-ntrl-outline);
			}
			&.solid {
				--icon-opacity: var(--opacity-btn-icon-solid);
				--label-clr: var(--clr-theme-ntrl-on-element);
				--btn-bg: var(--clr-theme-ntrl-element);

				/* if button */
				&:not(&:disabled):hover {
					--icon-opacity: var(--opacity-btn-icon-solid-hover);
					--btn-bg: var(--clr-theme-ntrl-element-hover);
				}
			}
		}

		.pop {
			&.outline,
			&.ghost {
				--label-clr: var(--clr-btn-pop-outline-text);
				--btn-bg: var(--clr-btn-pop-outline-bg);

				/* if button */
				&:not(&:disabled):hover,
				&.activated {
					--label-clr: var(--clr-btn-pop-outline-text-hover);
				}
			}
			&.outline {
				--btn-border-clr: var(--clr-btn-pop-outline);
			}
			&.solid {
				--icon-opacity: var(--opacity-btn-icon-solid);
				--label-clr: var(--clr-theme-pop-on-element);
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
			&.outline,
			&.ghost {
				--label-clr: var(--clr-btn-succ-outline-text);
				--btn-bg: var(--clr-btn-succ-outline-bg);

				/* if button */
				&:not(&:disabled):hover,
				&.activated {
					--label-clr: var(--clr-btn-succ-outline-text-hover);
				}
			}
			&.outline {
				--btn-border-clr: var(--clr-btn-succ-outline);
			}
			&.solid {
				--icon-opacity: var(--opacity-btn-icon-solid);
				--label-clr: var(--clr-theme-succ-on-element);
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
			&.outline,
			&.ghost {
				--label-clr: var(--clr-btn-err-outline-text);
				--btn-bg: var(--clr-btn-err-outline-bg);

				/* if button */
				&:not(&:disabled):hover,
				&.activated {
					--label-clr: var(--clr-btn-err-outline-text-hover);
				}
			}
			&.outline {
				--btn-border-clr: var(--clr-btn-err-outline);
			}
			&.solid {
				--icon-opacity: var(--opacity-btn-icon-solid);
				--label-clr: var(--clr-theme-err-on-element);
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
			&.outline,
			&.ghost {
				--label-clr: var(--clr-btn-warn-outline-text);
				--btn-bg: var(--clr-btn-warn-outline-bg);

				/* if button */
				&:not(&:disabled):hover,
				&.activated {
					--label-clr: var(--clr-btn-warn-outline-text-hover);
				}
			}
			&.outline {
				--btn-border-clr: var(--clr-btn-warn-outline);
			}
			&.solid {
				--icon-opacity: var(--opacity-btn-icon-solid);
				--label-clr: var(--clr-theme-warn-on-element);
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
			&.outline,
			&.ghost {
				--label-clr: var(--clr-btn-purp-outline-text);
				--btn-bg: var(--clr-btn-purp-outline-bg);

				/* if button */
				&:not(&:disabled):hover,
				&.activated {
					--label-clr: var(--clr-btn-purp-outline-text-hover);
				}
			}
			&.outline {
				--btn-border-clr: var(--clr-btn-purp-outline);
			}
			&.solid {
				--icon-opacity: var(--opacity-btn-icon-solid);
				--label-clr: var(--clr-theme-purp-on-element);
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
		.btn.icon-size {
			gap: 0;
			height: var(--size-icon);
			padding: 2px;
		}

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
			&.icon-size {
				width: var(--size-icon);
			}

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
	}
</style>
