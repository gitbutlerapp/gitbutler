<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
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
	let visible = false;

	export function show() {
		visible = true;
	}

	export function close() {
		visible = false;
	}

	let container: HTMLDivElement;
	let contextMenuContainer: HTMLDivElement;
	let iconEl: HTMLElement;
</script>

<div class="dropdown-wrapper" class:wide>
	<div class="dropdown" bind:this={container}>
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
			bind:element={iconEl}
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
			}}
		/>
	</div>
	<div
		class="context-menu-container"
		use:clickOutside={{
			trigger: iconEl,
			handler: () => (visible = false),
			enabled: visible
		}}
		bind:this={contextMenuContainer}
		style:display={visible ? 'block' : 'none'}
		class:dropdown-top={menuPosition === 'top'}
		class:dropdown-bottom={menuPosition === 'bottom'}
	>
		<slot name="context-menu" />
	</div>
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
