<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import Button from '$lib/shared/Button.svelte';
	import type iconsJson from '$lib/icons/icons.json';
	import type { ComponentColor, ComponentStyleKind } from '$lib/vbranches/types';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let style: ComponentColor = 'neutral';
	export let kind: ComponentStyleKind = 'soft';
	export let outline = false;
	export let disabled = false;
	export let loading = false;
	export let wide = false;
	export let help = '';
	export let menuPosition: 'top' | 'bottom' = 'bottom';

	let contextMenu: ContextMenu;
	let iconEl: HTMLElement;

	let visible = false;

	export function show() {
		visible = true;
		contextMenu.open();
	}

	export function close() {
		visible = false;
		contextMenu.close();
	}
</script>

<div class="dropdown-wrapper" class:wide>
	<div class="dropdown">
		<Button
			{style}
			{icon}
			{kind}
			{help}
			{outline}
			reversedDirection
			disabled={disabled || loading}
			isDropdownChild
			on:click
		>
			<slot />
		</Button>
		<Button
			bind:el={iconEl}
			{style}
			{kind}
			{help}
			{outline}
			icon={visible ? 'chevron-up' : 'chevron-down'}
			{loading}
			disabled={disabled || loading}
			isDropdownChild
			on:click={() => {
				visible = !visible;
				contextMenu.toggle();
			}}
		/>
	</div>
	<ContextMenu
		bind:this={contextMenu}
		target={iconEl}
		verticalAlign={menuPosition}
		onclose={() => {
			visible = false;
		}}
	>
		<slot name="context-menu" />
	</ContextMenu>
</div>

<style lang="postcss">
	.dropdown-wrapper {
		/* display set directly on element */
		position: relative;
	}

	.dropdown {
		display: flex;
		flex-grow: 1;
		align-items: center;
	}
	.wide {
		width: 100%;
	}
</style>
