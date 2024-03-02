<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
	import Button from '$lib/components/Button.svelte';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let color: 'primary' | 'neutral' | 'error' = 'primary';
	export let kind: 'filled' | 'outlined' = 'filled';
	export let disabled = false;
	export let loading = false;
	export let wide = false;
	export let help = '';
	let visible = false;

	export function show() {
		visible = true;
	}

	export function close() {
		visible = false;
	}

	let container: HTMLDivElement;
	let contextMenuContainer: HTMLDivElement;
	let iconElt: HTMLElement;
</script>

<div class="dropdown-wrapper" class:wide>
	<div class="dropdown" bind:this={container}>
		<Button
			{color}
			{icon}
			{kind}
			{help}
			iconAlign="left"
			disabled={disabled || loading}
			isDropdownChild
			on:click><slot /></Button
		>
		<Button
			class="dropdown__icon-btn"
			bind:element={iconElt}
			{color}
			{kind}
			{help}
			icon={visible ? 'chevron-top' : 'chevron-down'}
			{loading}
			disabled={disabled || loading}
			isDropdownChild
			on:click={() => (visible = !visible)}
		/>
	</div>
	<div
		class="context-menu-container"
		use:clickOutside={{
			trigger: iconElt,
			handler: () => (visible = !visible),
			enabled: visible
		}}
		bind:this={contextMenuContainer}
		style:display={visible ? 'block' : 'none'}
	>
		<slot name="context-menu" />
	</div>
</div>

<style lang="postcss">
	.dropdown-wrapper {
		/* display set directly on element */
		position: relative;
	}

	.dropdown-wrapper :global(.dropdown__text-btn) {
		z-index: 1;
		border-top-right-radius: 0;
		border-bottom-right-radius: 0;

		&:hover {
			z-index: 2;
		}
	}

	.dropdown-wrapper :global(.dropdown__icon-btn) {
		z-index: 1;
		border-top-left-radius: 0;
		border-bottom-left-radius: 0;

		&:hover {
			z-index: 2;
		}
	}

	.dropdown {
		display: flex;
		flex-grow: 1;
		align-items: center;
	}

	.context-menu-container {
		position: absolute;
		right: 0;
		bottom: 100%;
		padding-bottom: var(--space-4);
		z-index: 50;
	}

	.wide {
		width: 100%;
	}
</style>
