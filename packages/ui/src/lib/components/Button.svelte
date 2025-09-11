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
		tooltipMaxWidth?: number;
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
		tooltipMaxWidth,
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
	maxWidth={tooltipMaxWidth}
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
		:where(&.outline:not(.neutral)) {
			--opacity-btn-bg: 0.1;
			--icon-opacity: var(--opacity-btn-icon-outline);
		}

		/* Ghost buttons and neutral outline buttons keep transparent background by default */
		:where(&.ghost),
		:where(&.outline.neutral) {
			--opacity-btn-bg: 0;
			--icon-opacity: var(--opacity-btn-icon-outline);
		}

		/* Outline buttons (except neutral) hover with darker background */
		:where(&.outline:not(.neutral):not(:disabled):hover),
		:where(&.outline:not(.neutral).activated) {
			--icon-opacity: var(--opacity-btn-icon-outline-hover);
			--opacity-btn-bg: 0.25;
		}

		/* Neutral outline and ghost buttons hover */
		:where(&.outline.neutral:not(:disabled):hover),
		:where(&.ghost:not(:disabled):hover),
		:where(&.outline.neutral.activated),
		:where(&.ghost.activated) {
			--icon-opacity: var(--opacity-btn-icon-outline-hover);
			--opacity-btn-bg: var(--opacity-btn-outline-bg-hover);
		}

		:where(&.outline) {
			border: 1px solid
				color-mix(
					in srgb,
					var(--btn-border-clr, transparent),
					transparent calc((1 - var(--opacity-btn-outline, 1)) * 100%)
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

		/* Theme Variables - All themes use the same pattern */
		:where(&.neutral) {
			--theme-outline-text: var(--clr-btn-ntrl-outline-text);
			--theme-outline-text-hover: var(--clr-btn-ntrl-outline-text-hover);
			--theme-outline-bg: var(--clr-btn-ntrl-outline-bg);
			--theme-outline-border: var(--clr-btn-ntrl-outline);
			--theme-solid-text: var(--clr-theme-ntrl-on-element);
			--theme-solid-bg: var(--clr-theme-ntrl-element);
			--theme-solid-bg-hover: var(--clr-theme-ntrl-element-hover);
		}

		:where(&.pop) {
			--theme-outline-text: var(--clr-btn-pop-outline-text);
			--theme-outline-text-hover: var(--clr-btn-pop-outline-text-hover);
			--theme-outline-bg: var(--clr-btn-pop-outline-bg);
			--theme-outline-border: var(--clr-btn-pop-outline);
			--theme-solid-text: var(--clr-theme-pop-on-element);
			--theme-solid-bg: var(--clr-theme-pop-element);
			--theme-solid-bg-hover: var(--clr-theme-pop-element-hover);
		}

		:where(&.success) {
			--theme-outline-text: var(--clr-btn-succ-outline-text);
			--theme-outline-text-hover: var(--clr-btn-succ-outline-text-hover);
			--theme-outline-bg: var(--clr-btn-succ-outline-bg);
			--theme-outline-border: var(--clr-btn-succ-outline);
			--theme-solid-text: var(--clr-theme-succ-on-element);
			--theme-solid-bg: var(--clr-theme-succ-element);
			--theme-solid-bg-hover: var(--clr-theme-succ-element-hover);
		}

		:where(&.error) {
			--theme-outline-text: var(--clr-btn-err-outline-text);
			--theme-outline-text-hover: var(--clr-btn-err-outline-text-hover);
			--theme-outline-bg: var(--clr-btn-err-outline-bg);
			--theme-outline-border: var(--clr-btn-err-outline);
			--theme-solid-text: var(--clr-theme-err-on-element);
			--theme-solid-bg: var(--clr-theme-err-element);
			--theme-solid-bg-hover: var(--clr-theme-err-element-hover);
		}

		:where(&.warning) {
			--theme-outline-text: var(--clr-btn-warn-outline-text);
			--theme-outline-text-hover: var(--clr-btn-warn-outline-text-hover);
			--theme-outline-bg: var(--clr-btn-warn-outline-bg);
			--theme-outline-border: var(--clr-btn-warn-outline);
			--theme-solid-text: var(--clr-theme-warn-on-element);
			--theme-solid-bg: var(--clr-theme-warn-element);
			--theme-solid-bg-hover: var(--clr-theme-warn-element-hover);
		}

		:where(&.purple) {
			--theme-outline-text: var(--clr-btn-purp-outline-text);
			--theme-outline-text-hover: var(--clr-btn-purp-outline-text-hover);
			--theme-outline-bg: var(--clr-btn-purp-outline-bg);
			--theme-outline-border: var(--clr-btn-purp-outline);
			--theme-solid-text: var(--clr-theme-purp-on-element);
			--theme-solid-bg: var(--clr-theme-purp-element);
			--theme-solid-bg-hover: var(--clr-theme-purp-element-hover);
		}

		/* Apply patterns using consolidated theme variables */
		:where(&.outline),
		:where(&.ghost) {
			--label-clr: var(--theme-outline-text);
			--btn-bg: var(--theme-outline-bg);
		}

		:where(&.outline:not(:disabled):hover),
		:where(&.ghost:not(:disabled):hover),
		:where(&.outline.activated),
		:where(&.ghost.activated) {
			--label-clr: var(--theme-outline-text-hover);
		}

		:where(&.outline) {
			--btn-border-clr: var(--theme-outline-border);
		}

		:where(&.solid) {
			--icon-opacity: var(--opacity-btn-icon-solid);
			--label-clr: var(--theme-solid-text);
			--btn-bg: var(--theme-solid-bg);
		}

		:where(&.solid:not(:disabled):hover),
		:where(&.solid.activated) {
			--icon-opacity: var(--opacity-btn-icon-solid-hover);
			--btn-bg: var(--theme-solid-bg-hover);
		}

		/* Size modifiers with size variables */
		&.icon-size {
			--btn-size: var(--size-icon);
			--btn-padding: 2px;
			--btn-gap: 0;
		}

		&.tag-size {
			--btn-size: var(--size-tag);
			--btn-padding: 2px 4px;
			--btn-gap: 0;
		}

		&.button-size {
			--btn-size: var(--size-button);
			--btn-padding: 4px 6px;
			--btn-gap: 2px;
		}

		&.cta-size {
			--btn-size: var(--size-cta);
			--btn-padding: 6px 8px;
			--btn-gap: 2px;
		}

		/* Apply size variables */
		&[class*='-size'] {
			height: var(--btn-size);
			padding: var(--btn-padding);
			gap: var(--btn-gap);
		}

		/* Fixed width variants */
		&.fixed-width[class*='-size'] {
			width: var(--btn-size);
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
