<script lang="ts" module>
	export interface Props {
		id?: string | undefined;
		el?: HTMLElement;
		// Interaction props
		disabled?: boolean;
		loading?: boolean;
		activated?: boolean;
		tabindex?: number | undefined;
		type?: "submit" | "reset" | "button" | undefined;
		autofocus?: boolean;
		// Layout props
		shrinkable?: boolean;
		reversedDirection?: boolean;
		width?: number | string | undefined;
		size?: "tag" | "button";
		wide?: boolean;
		grow?: boolean;
		align?: "flex-start" | "center" | "flex-end" | "stretch" | "baseline" | "auto";
		dropdownChild?: boolean;
		// Style props
		style?: ComponentColorType;
		kind?: ComponentKindType;
		class?: string | (string | undefined)[] | Record<string, string>;
		iconClass?: string;
		customStyle?: string;
		// Additional elements
		icon?: IconName;
		isDropdown?: boolean;
		dropdownOpen?: boolean;
		hotkey?: string;
		tooltip?: string;
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
		badge?: Snippet;
	}
</script>

<script lang="ts">
	import Icon from "$components/Icon.svelte";
	import Tooltip, { type TooltipAlign, type TooltipPosition } from "$components/Tooltip.svelte";
	import { focusable } from "$lib/focus/focusable";
	import { type IconName } from "$lib/icons/names";
	import { formatHotkeyForPlatform } from "$lib/utils/hotkeySymbols";
	import { pxToRem } from "$lib/utils/pxToRem";
	import { onMount, tick } from "svelte";
	import type { ComponentColorType, ComponentKindType } from "$lib/utils/colorTypes";
	import type { Snippet } from "svelte";

	let {
		el = $bindable(),
		disabled = false,
		id = undefined,
		loading = false,
		activated = false,
		tabindex,
		type = "button",
		autofocus = false,
		shrinkable = false,
		reversedDirection = false,
		width,
		size = "button",
		wide = false,
		grow = false,
		align = "auto",
		dropdownChild = false,
		style = "gray",
		kind = "solid",
		hotkey,
		class: className = "",
		iconClass = "",
		customStyle,
		testId,
		icon,
		isDropdown = false,
		dropdownOpen = false,
		tooltip,
		tooltipPosition,
		tooltipAlign,
		tooltipDelay,
		onclick,
		onmousedown: onmousedownExternal,
		oncontextmenu,
		onkeydown,
		children,
		custom,
		badge,
	}: Props = $props();

	async function handleAction(e: MouseEvent) {
		if (loading || disabled) {
			e.preventDefault();
			e.stopPropagation();
			return;
		}

		await onclick?.(e);
	}

	const hasChildren = $derived(Boolean(children));
	const isDisabled = $derived(disabled || loading);
	const resolvedWidth = $derived(
		width !== undefined ? (typeof width === "number" ? `${pxToRem(width)}rem` : width) : undefined,
	);

	const buttonClasses = $derived([
		"btn",
		style,
		kind,
		size && `${size}-size`,
		activated && "activated",
		grow && "grow",
		wide && "wide",
		shrinkable && "shrinkable",
		reversedDirection && "reversed-direction",
		dropdownChild && "is-dropdown",
		!hasChildren && !wide && "fixed-width",
		className,
	]);

	const displayHotkey = $derived(hotkey ? formatHotkeyForPlatform(hotkey) : undefined);
	let tooltipInstance = $state<Tooltip>();

	function onmousedown(e: MouseEvent) {
		tooltipInstance?.dismiss();
		onmousedownExternal?.(e);
	}

	onMount(() => {
		if (autofocus) {
			tick().then(() => {
				el?.focus();
			});
		}
	});
</script>

<Tooltip
	bind:this={tooltipInstance}
	text={tooltip}
	align={tooltipAlign}
	position={tooltipPosition}
	delay={tooltipDelay}
	hotkey={displayHotkey}
>
	<button
		bind:this={el}
		use:focusable={{
			button: true,
			hotkey: hotkey,
			onAction: () => {
				el?.click();
			},
		}}
		class={buttonClasses}
		style:align-self={align}
		style:width={resolvedWidth}
		style={customStyle}
		disabled={isDisabled}
		onclick={handleAction}
		{onmousedown}
		{oncontextmenu}
		{onkeydown}
		{type}
		{id}
		{tabindex}
		{...testId ? { "data-testid": testId } : {}}
	>
		{#if children}
			<span class="btn-label text-semibold truncate text-12">
				{@render children()}

				{#if displayHotkey}
					<span class="btn-hotkey">
						{displayHotkey}
					</span>
				{/if}
				{#if badge}
					<span class="btn-badge">
						{@render badge()}
					</span>
				{/if}
			</span>
		{/if}

		{#if icon || loading || isDropdown}
			<div class={["btn-icon", iconClass]}>
				{#if loading}
					<Icon name="spinner" />
				{:else if isDropdown}
					<div class="btn-dropdown-chevron" class:open={dropdownOpen}>
						<Icon name="chevron-down" />
					</div>
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
	:where(.btn) {
		/* ---- Shared mix ratios ---- */
		--_outline-base-mix: 0%;

		/* ---- Base layout ---- */
		display: inline-flex;
		position: relative;
		align-items: center;
		justify-content: center;
		transform-style: preserve-3d;
		border-radius: var(--radius-btn);
		backface-visibility: hidden;
		background: var(--btn-bg);
		color: var(--label-clr);
		cursor: pointer;

		transition:
			background var(--transition-fast),
			opacity var(--transition-fast),
			color var(--transition-fast),
			max-width var(--transition-medium);
		user-select: none;

		/* ---- Child elements ---- */
		.btn-label {
			padding: 0 3px;
			overflow: hidden;
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

		.btn-badge {
			display: inline-flex;
			margin-right: -2px;
			margin-left: 2px;
		}

		.btn-hotkey {
			margin-left: 2px;
			opacity: 0.5;
		}

		/* ---- Theme tokens ---- */
		:where(&.gray) {
			--_solid-bg: var(--clr-theme-gray-element);
			--_solid-fg: var(--clr-theme-gray-on-element);
			--_outline-text: var(--clr-btn-gray-outline-text);
			--_outline-bg: var(--clr-btn-gray-outline-bg);
			--_outline-border: var(--clr-btn-gray-outline);
			--_focus-ring: var(--clr-theme-pop-element);
			--_focus-solid-mix: 100%;
		}

		:where(&.pop) {
			--_solid-bg: var(--clr-theme-pop-element);
			--_solid-fg: var(--clr-theme-pop-on-element);
			--_outline-text: var(--clr-btn-pop-outline-text);
			--_outline-bg: var(--clr-btn-pop-outline-bg);
			--_outline-border: var(--clr-btn-pop-outline);
			--_focus-ring: var(--clr-theme-pop-element);
		}

		:where(&.safe) {
			--_solid-bg: var(--clr-theme-safe-element);
			--_solid-fg: var(--clr-theme-safe-on-element);
			--_outline-text: var(--clr-btn-safe-outline-text);
			--_outline-bg: var(--clr-btn-safe-outline-bg);
			--_outline-border: var(--clr-btn-safe-outline);
			--_focus-ring: var(--clr-theme-safe-element);
		}

		:where(&.danger) {
			--_solid-bg: var(--clr-theme-danger-element);
			--_solid-fg: var(--clr-theme-danger-on-element);
			--_outline-text: var(--clr-btn-danger-outline-text);
			--_outline-bg: var(--clr-btn-danger-outline-bg);
			--_outline-border: var(--clr-btn-danger-outline);
			--_focus-ring: var(--clr-theme-danger-element);
		}

		:where(&.warning) {
			--_solid-bg: var(--clr-theme-warn-element);
			--_solid-fg: var(--clr-theme-warn-on-element);
			--_outline-text: var(--clr-btn-warn-outline-text);
			--_outline-bg: var(--clr-btn-warn-outline-bg);
			--_outline-border: var(--clr-btn-warn-outline);
			--_focus-ring: var(--clr-theme-warn-element);
		}

		:where(&.purple) {
			--_solid-bg: var(--clr-theme-purple-element);
			--_solid-fg: var(--clr-theme-purple-on-element);
			--_outline-text: var(--clr-btn-purple-outline-text);
			--_outline-bg: var(--clr-btn-purple-outline-bg);
			--_outline-border: var(--clr-btn-purple-outline);
			--_focus-ring: var(--clr-theme-purple-element);
		}

		/* ---- Variants ---- */
		:where(&.solid) {
			--icon-opacity: var(--opacity-mix-btn-solid-icon);
			--label-clr: var(--_solid-fg);
			--btn-bg: var(--_solid-bg);
			--_solid-hover-bg: color-mix(
				in srgb,
				var(--_solid-bg),
				var(--clr-core-gray-0) var(--opacity-mix-btn-solid-hover)
			);
		}

		:where(&.outline),
		:where(&.ghost) {
			--icon-opacity: var(--opacity-mix-btn-outline-icon);
			--label-clr: var(--_outline-text);
		}

		:where(&.outline) {
			--btn-bg: color-mix(in srgb, var(--_outline-bg) var(--_outline-base-mix), transparent);
			border: 1px solid
				color-mix(
					in srgb,
					var(--_outline-border) var(--opacity-mix-btn-outline-border),
					transparent
				);
		}

		:where(&.outline:not(.gray)) {
			--_outline-base-mix: 10%;
		}

		:where(&.ghost) {
			--btn-bg: transparent;
		}

		:where(&.solid:not(:disabled):hover),
		:where(&.solid.activated) {
			--btn-bg: var(--_solid-hover-bg);
		}

		:where(&.outline:not(:disabled):hover),
		:where(&.ghost:not(:disabled):hover),
		:where(&.outline.activated),
		:where(&.ghost.activated) {
			--btn-bg: color-mix(
				in srgb,
				var(--_outline-bg) var(--opacity-mix-btn-outline-bg-hover),
				transparent
			);
		}

		/* ---- Focus ---- */
		:where(&.outline:focus-visible),
		:where(&.ghost:focus-visible) {
			outline: 2px solid var(--_focus-ring);
			outline-offset: -2px;
		}

		:where(&.solid:focus-visible) {
			outline: 2px solid
				color-mix(in srgb, var(--_focus-ring) var(--_focus-solid-mix, 50%), var(--clr-text-1));
			outline-offset: -2px;
		}

		/* ---- Sizes ---- */
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

		&[class*="-size"] {
			height: var(--btn-size);
			padding: var(--btn-padding);
			gap: var(--btn-gap);
		}

		&.fixed-width[class*="-size"] {
			width: var(--btn-size);
		}

		/* ---- Dropdown split-button ---- */
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
			content: "";
			opacity: 0.4;
		}

		&.is-dropdown:last-of-type {
			border-top-left-radius: 0;
			border-bottom-left-radius: 0;
		}

		/* ---- State + layout modifiers ---- */
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
			width: fit-content;
			overflow: hidden;
		}

		&.shrinkable .btn-label {
			display: inline-block;
			overflow: hidden;
			text-overflow: ellipsis;
		}
	}

	.btn-dropdown-chevron {
		display: flex;
		transition: transform 0.15s ease;

		&.open {
			transform: rotate(180deg);
		}
	}
</style>
