<script lang="ts">
	import Button from "$components/Button.svelte";
	import ContextMenu from "$components/ContextMenu.svelte";
	import Tooltip from "$components/Tooltip.svelte";
	import { type IconName } from "$lib/icons/names";
	import type { ComponentColorType, ComponentKindType } from "$lib/utils/colorTypes";
	import type { Snippet } from "svelte";

	interface Props {
		testId?: string;
		icon?: IconName;
		style?: ComponentColorType;
		kind?: ComponentKindType;
		disabled?: boolean;
		loading?: boolean;
		wide?: boolean;
		grow?: boolean;
		width?: number | string;
		autoClose?: boolean;
		tooltip?: string;
		type?: "button" | "submit" | "reset";
		menuSide?: "top" | "bottom" | "left" | "right";
		shrinkable?: boolean;
		hotkey?: string;
		children?: Snippet;
		contextMenuSlot: Snippet;
		onclick?: (e: MouseEvent) => void;
	}

	const {
		testId,
		icon,
		style = "gray",
		kind = "solid",
		disabled = false,
		loading = false,
		wide = false,
		grow = false,
		width,
		autoClose = false,
		type,
		menuSide,
		tooltip,
		shrinkable,
		hotkey,
		children,
		contextMenuSlot,
		onclick,
	}: Props = $props();

	let iconEl = $state<HTMLElement>();
	let visible = $state(false);

	function preventContextMenu(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
	}

	export function show() {
		visible = true;
	}

	export function close() {
		visible = false;
	}
</script>

<Tooltip text={tooltip}>
	<div class="dropdown" class:wide class:grow class:shrinkable>
		<Button
			{testId}
			{style}
			{icon}
			{kind}
			{shrinkable}
			{type}
			{width}
			{hotkey}
			reversedDirection
			disabled={disabled || loading}
			dropdownChild
			{onclick}
			oncontextmenu={preventContextMenu}
			{children}
		></Button>
		<Button
			bind:el={iconEl}
			{style}
			{kind}
			isDropdown
			dropdownOpen={visible}
			{loading}
			disabled={disabled || loading}
			dropdownChild
			onclick={() => {
				visible = !visible;
			}}
			oncontextmenu={preventContextMenu}
		/>
	</div>
	{#if visible}
		<ContextMenu
			target={iconEl}
			leftClickTrigger={iconEl}
			side={menuSide}
			onclose={() => {
				visible = false;
			}}
			onclick={() => {
				if (autoClose) {
					visible = false;
				}
			}}
			onkeypress={() => {
				if (autoClose) {
					visible = false;
				}
			}}
		>
			{@render contextMenuSlot()}
		</ContextMenu>
	{/if}
</Tooltip>

<style lang="postcss">
	.dropdown {
		display: flex;
		position: relative;
		align-items: center;

		&.grow {
			flex-grow: 1;
		}
		&.wide {
			width: 100%;
		}
		&.shrinkable {
			overflow: hidden;
		}
	}
</style>
