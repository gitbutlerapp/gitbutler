<script lang="ts">
	import ContextMenuNew from '$lib/components/contextmenu/ContextMenuNew.svelte';
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

	let contextMenu: ContextMenuNew;
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
	<ContextMenuNew bind:this={contextMenu} target={iconEl} verticalAlign={menuPosition}>
		<slot name="context-menu" />
	</ContextMenuNew>
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

	.context-menu-container {
		position: absolute;
		right: 0;
		z-index: var(--z-floating);
	}

	.dropdown-top {
		bottom: 100%;
		padding-bottom: 4px;
	}

	.dropdown-bottom {
		top: 100%;
		padding-top: 4px;
	}

	.wide {
		width: 100%;
	}
</style>
