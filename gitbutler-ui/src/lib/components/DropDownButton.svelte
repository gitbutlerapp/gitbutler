<script lang="ts">
	import { joinClassNames } from '$lib/utils/joinClassNames';
	import Button from '$lib/components/Button.svelte';
	import { clickOutside } from '$lib/clickOutside';
	import type iconsJson from '$lib/icons/icons.json';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let color: 'primary' | 'neutral' | 'error' = 'primary';
	export let kind: 'filled' | 'outlined' = 'filled';
	export let disabled = false;
	export let loading = false;
	export let wide = false;
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
			class={joinClassNames([
				'dropdown__text-btn',
				kind == 'outlined' ? 'dropdown__text-btn_outlined' : 'dropdown__text-btn_filled',
				wide ? 'wide-text-btn' : ''
			])}
			{color}
			{icon}
			{kind}
			iconAlign="left"
			disabled={disabled || loading}
			on:click><slot /></Button
		>
		<Button
			class={joinClassNames([
				'dropdown__icon-btn',
				kind == 'outlined' ? 'dropdown__icon-btn_outlined' : ''
			])}
			bind:element={iconElt}
			{color}
			{kind}
			icon={visible ? 'chevron-top' : 'chevron-down'}
			{loading}
			disabled={disabled || loading}
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

	.dropdown-wrapper :global(.dropdown__text-btn_outlined) {
		transform: translateX(1px);
	}

	.dropdown-wrapper :global(.dropdown__text-btn_filled) {
		border-right: 1px solid var(--clr-theme-scale-pop-50);
	}

	.dropdown-wrapper :global(.dropdown__icon-btn_outlined):disabled {
		border-left: 1px solid transparent;
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

	.dropdown-wrapper :global(.wide-text-btn) {
		flex: 1;
	}
</style>
