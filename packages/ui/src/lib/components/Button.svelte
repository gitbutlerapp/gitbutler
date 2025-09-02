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
		customStyle?: string;
		// Additional elements
		icon?: keyof typeof iconsJson | undefined;
		hotkey?: string;
		tooltip?: string;
		tooltipHotkey?: string;
		tooltipPosition?: TooltipPosition;
		tooltipAlign?: TooltipAlign;
		tooltipDelay?: number;
		testId?: string;
		// Events
		onclick?: ((e: MouseEvent) => Promise<void>) | ((e: MouseEvent) => void);
		onmousedown?: (e: MouseEvent) => void;
		oncontextmenu?: (e: MouseEvent) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		// Snippets
		children?: Snippet;
		custom?: Snippet;
	}
</script>

<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import Tooltip, { type TooltipAlign, type TooltipPosition } from '$components/Tooltip.svelte';
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
		hotkey,
		solidBackground = false,
		class: className = '',
		iconClass = '',
		customStyle,
		testId,
		icon,
		tooltip,
		tooltipHotkey,
		tooltipPosition,
		tooltipAlign,
		tooltipDelay,
		onclick,
		onmousedown,
		oncontextmenu,
		onkeydown,
		children,
		custom
	}: Props = $props();

	async function handleAction(e: MouseEvent) {
		if (loading || disabled) {
			e.preventDefault();
			e.stopPropagation();
		} else {
			await onclick?.(e);
		}
	}
</script>

<Tooltip
	text={tooltip}
	align={tooltipAlign}
	position={tooltipPosition}
	delay={tooltipDelay}
	hotkey={tooltipHotkey}
>
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
				? `${pxToRem(width)}rem`
				: width
			: undefined}
		style:max-width={maxWidth !== undefined ? `${pxToRem(maxWidth)}rem` : undefined}
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
				class="btn-label text-semibold truncate"
				class:text-12={size === 'button' || size === 'cta'}
				class:text-11={size === 'tag'}
				class:text-10={size === 'icon'}
			>
				{@render children()}

				{#if hotkey}
					<span class="btn-hotkey">
						{hotkey}
					</span>
				{/if}
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

		{#if custom}
			{@render custom()}
		{/if}
	</button>
</Tooltip>

<style lang="postcss">
	/* Global variables for button styles */
	/* :where approach applies a lower specificity to the styles,
	   allowing for easier overrides and better maintainability.
	 */
	:where(.btn) {
		display: inline-flex;
		position: relative;
		align-items: center;
		justify-content: center;
		transform-style: preserve-3d;
		border-radius: var(--radius-btn);
		backface-visibility: hidden;
		background: color-mix(
			in srgb,
			var(--btn-bg, transparent),
			transparent calc((1 - var(--opacity-btn-bg, 1)) * 100%)
		);
		color: var(--label-clr);
		cursor: pointer;

		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast),
			max-width var(--transition-medium);
		user-select: none;

		/* Consolidated outline and ghost styles */

		/* All outline buttons except neutral get a slight background by default */
		&.outline:not(.neutral) {
			--opacity-btn-bg: 0.1;
			--icon-opacity: var(--opacity-btn-icon-outline);
		}

		/* Ghost buttons and neutral outline buttons keep transparent background by default */
		&.ghost,
		&.outline.neutral {
			--opacity-btn-bg: 0;
			--icon-opacity: var(--opacity-btn-icon-outline);
		}

		/* Outline buttons (except neutral) hover with darker background */
		&.outline:not(.neutral):not(:disabled):hover,
		&.outline:not(.neutral).activated {
			--icon-opacity: var(--opacity-btn-icon-outline-hover);
			--opacity-btn-bg: 0.25;
		}

		/* Neutral outline and ghost buttons hover */
		&.outline.neutral:not(:disabled):hover,
		&.ghost:not(:disabled):hover,
		&.outline.neutral.activated,
		&.ghost.activated {
			--icon-opacity: var(--opacity-btn-icon-outline-hover);
			--opacity-btn-bg: var(--opacity-btn-outline-bg-hover);
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

		/* Child elements */
		.btn-label {
			padding: 0 3px;
			white-space: nowrap;
			pointer-events: none;
		}

		.btn-icon {
			display: flex;
			flex-shrink: 0;
			transform: translateZ(0); /* Safari flickering fix */
			opacity: var(--icon-opacity);
			pointer-events: none;
			transition: opacity var(--transition-fast);
		}

		.btn-hotkey {
			margin-left: 2px;
			opacity: 0.5;
			pointer-events: none;
		}

		/* Neutral theme */
		&.neutral.outline,
		&.neutral.ghost {
			--label-clr: var(--clr-btn-ntrl-outline-text);
			--btn-bg: var(--clr-btn-ntrl-outline-bg);
		}

		&.neutral.outline:not(:disabled):hover,
		&.neutral.ghost:not(:disabled):hover,
		&.neutral.outline.activated,
		&.neutral.ghost.activated {
			--label-clr: var(--clr-btn-ntrl-outline-text-hover);
		}

		&.neutral.outline {
			--btn-border-clr: var(--clr-btn-ntrl-outline);
		}

		&.neutral.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--label-clr: var(--clr-theme-ntrl-on-element);
			--btn-bg: var(--clr-theme-ntrl-element);
		}

		&.neutral.solid:not(:disabled):hover {
			--icon-opacity: var(--opacity-btn-icon-solid-hover);
			--btn-bg: var(--clr-theme-ntrl-element-hover);
		}

		/* Pop theme */
		&.pop.outline,
		&.pop.ghost {
			--label-clr: var(--clr-btn-pop-outline-text);
			--btn-bg: var(--clr-btn-pop-outline-bg);
		}

		&.pop.outline:not(:disabled):hover,
		&.pop.ghost:not(:disabled):hover,
		&.pop.outline.activated,
		&.pop.ghost.activated {
			--label-clr: var(--clr-btn-pop-outline-text-hover);
		}

		&.pop.outline {
			--btn-border-clr: var(--clr-btn-pop-outline);
		}

		&.pop.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--label-clr: var(--clr-theme-pop-on-element);
			--btn-bg: var(--clr-theme-pop-element);
		}

		&.pop.solid:not(:disabled):hover,
		&.pop.solid.activated {
			--icon-opacity: var(--opacity-btn-icon-solid-hover);
			--btn-bg: var(--clr-theme-pop-element-hover);
		}

		/* Success theme */
		&.success.outline,
		&.success.ghost {
			--label-clr: var(--clr-btn-succ-outline-text);
			--btn-bg: var(--clr-btn-succ-outline-bg);
		}

		&.success.outline:not(:disabled):hover,
		&.success.ghost:not(:disabled):hover,
		&.success.outline.activated,
		&.success.ghost.activated {
			--label-clr: var(--clr-btn-succ-outline-text-hover);
		}

		&.success.outline {
			--btn-border-clr: var(--clr-btn-succ-outline);
		}

		&.success.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--label-clr: var(--clr-theme-succ-on-element);
			--btn-bg: var(--clr-theme-succ-element);
		}

		&.success.solid:not(:disabled):hover,
		&.success.solid.activated {
			--icon-opacity: var(--opacity-btn-icon-solid-hover);
			--btn-bg: var(--clr-theme-succ-element-hover);
		}

		/* Error theme */
		&.error.outline,
		&.error.ghost {
			--label-clr: var(--clr-btn-err-outline-text);
			--btn-bg: var(--clr-btn-err-outline-bg);
		}

		&.error.outline:not(:disabled):hover,
		&.error.ghost:not(:disabled):hover,
		&.error.outline.activated,
		&.error.ghost.activated {
			--label-clr: var(--clr-btn-err-outline-text-hover);
		}

		&.error.outline {
			--btn-border-clr: var(--clr-btn-err-outline);
		}

		&.error.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--label-clr: var(--clr-theme-err-on-element);
			--btn-bg: var(--clr-theme-err-element);
		}

		&.error.solid:not(:disabled):hover,
		&.error.solid.activated {
			--icon-opacity: var(--opacity-btn-icon-solid-hover);
			--btn-bg: var(--clr-theme-err-element-hover);
		}

		/* Warning theme */
		&.warning.outline,
		&.warning.ghost {
			--label-clr: var(--clr-btn-warn-outline-text);
			--btn-bg: var(--clr-btn-warn-outline-bg);
		}

		&.warning.outline:not(:disabled):hover,
		&.warning.ghost:not(:disabled):hover,
		&.warning.outline.activated,
		&.warning.ghost.activated {
			--label-clr: var(--clr-btn-warn-outline-text-hover);
		}

		&.warning.outline {
			--btn-border-clr: var(--clr-btn-warn-outline);
		}

		&.warning.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--label-clr: var(--clr-theme-warn-on-element);
			--btn-bg: var(--clr-theme-warn-element);
		}

		&.warning.solid:not(:disabled):hover,
		&.warning.solid.activated {
			--icon-opacity: var(--opacity-btn-icon-solid-hover);
			--btn-bg: var(--clr-theme-warn-element-hover);
		}

		/* Purple theme */
		&.purple.outline,
		&.purple.ghost {
			--label-clr: var(--clr-btn-purp-outline-text);
			--btn-bg: var(--clr-btn-purp-outline-bg);
		}

		&.purple.outline:not(:disabled):hover,
		&.purple.ghost:not(:disabled):hover,
		&.purple.outline.activated,
		&.purple.ghost.activated {
			--label-clr: var(--clr-btn-purp-outline-text-hover);
		}

		&.purple.outline {
			--btn-border-clr: var(--clr-btn-purp-outline);
		}

		&.purple.solid {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--label-clr: var(--clr-theme-purp-on-element);
			--btn-bg: var(--clr-theme-purp-element);
		}

		&.purple.solid:not(:disabled):hover,
		&.purple.solid.activated {
			--icon-opacity: var(--opacity-btn-icon-solid-hover);
			--btn-bg: var(--clr-theme-purp-element-hover);
		}

		/* Size modifiers */
		&.icon-size {
			height: var(--size-icon);
			padding: 2px;
			gap: 0;
		}

		&.tag-size {
			height: var(--size-tag);
			padding: 2px 4px;
			gap: 0;
		}

		&.button-size {
			height: var(--size-button);
			padding: 4px 6px;
			gap: 2px;
		}

		&.cta-size {
			height: var(--size-cta);
			padding: 6px 8px;
			gap: 2px;
		}

		/* Fixed width variants */
		&.fixed-width.icon-size {
			width: var(--size-icon);
		}

		&.fixed-width.tag-size {
			width: var(--size-tag);
		}

		&.fixed-width.button-size {
			width: var(--size-button);
		}

		&.fixed-width.cta-size {
			width: var(--size-cta);
		}

		/* Dropdown styles */
		&.is-dropdown:first-of-type {
			flex: 1;
			border-right: none;
			border-top-right-radius: 0;
			border-bottom-right-radius: 0;
		}

		&.is-dropdown:first-of-type.solid:after {
			z-index: var(--z-lifted);
			position: absolute;
			top: 0;
			right: 0;
			width: 1px;
			height: 100%;
			background-color: currentColor;
			content: '';
			opacity: 0.4;
		}

		&.is-dropdown:last-of-type {
			border-top-left-radius: 0;
			border-bottom-left-radius: 0;
		}

		/* State modifiers */
		&:disabled {
			cursor: not-allowed;
			opacity: 0.5;
		}

		/* Layout modifiers */
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
			width: fit-content;
			overflow: hidden;
		}

		&.shrinkable .label {
			display: inline-block;
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}
	/* See `tabbable.ts` for more on this class. */
	:global(.focus-visible) {
		outline: 2px solid var(--clr-theme-pop-element-hover);
	}
</style>
