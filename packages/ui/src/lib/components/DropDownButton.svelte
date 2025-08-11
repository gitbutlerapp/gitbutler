<script lang="ts">
	import Button from '$components/Button.svelte';
	import ContextMenu from '$components/ContextMenu.svelte';
	import Tooltip from '$components/Tooltip.svelte';
	import type iconsJson from '$lib/data/icons.json';
	import type { ComponentColorType, ComponentKindType } from '$lib/utils/colorTypes';
	import type { Snippet } from 'svelte';

	interface Props {
		testId?: string;
		icon?: keyof typeof iconsJson;
		style?: ComponentColorType;
		kind?: ComponentKindType;
		disabled?: boolean;
		loading?: boolean;
		wide?: boolean;
		grow?: boolean;
		width?: number | string;
		autoClose?: boolean;
		tooltip?: string;
		type?: 'button' | 'submit' | 'reset';
		menuPosition?: 'top' | 'bottom';
		shrinkable?: boolean;
		children?: Snippet;
		contextMenuSlot: Snippet;
		onclick?: (e: MouseEvent) => void;
	}

	const {
		testId,
		icon,
		style = 'neutral',
		kind = 'solid',
		disabled = false,
		loading = false,
		wide = false,
		grow = false,
		width,
		autoClose = false,
		type,
		tooltip,
		menuPosition = 'bottom',
		shrinkable,
		children,
		contextMenuSlot,
		onclick
	}: Props = $props();

	let contextMenu = $state<ReturnType<typeof ContextMenu>>();
	let iconEl = $state<HTMLElement>();
	let visible = $state(false);

	function preventContextMenu(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
	}

	export function show() {
		visible = true;
		contextMenu?.open();
	}

	export function close() {
		visible = false;
		contextMenu?.close();
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
			icon={visible ? 'chevron-up' : 'chevron-down'}
			{loading}
			disabled={disabled || loading}
			dropdownChild
			onclick={() => {
				visible = !visible;
				contextMenu?.toggle();
			}}
			oncontextmenu={preventContextMenu}
		/>
	</div>
	<ContextMenu
		bind:this={contextMenu}
		leftClickTrigger={iconEl}
		verticalAlign={menuPosition}
		onclose={() => {
			visible = false;
		}}
		onclick={() => {
			if (autoClose) {
				contextMenu?.close();
			}
		}}
		onkeypress={() => {
			if (autoClose) {
				contextMenu?.close();
			}
		}}
	>
		{@render contextMenuSlot()}
	</ContextMenu>
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
