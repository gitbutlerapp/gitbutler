<script lang="ts">
	import { onMount } from 'svelte';
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from '$lib/icons/Icon.svelte';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let iconAlign: 'left' | 'right' = 'right';
	export let color: 'primary' | 'neutral' | 'error' = 'primary';
	export let kind: 'filled' | 'outlined' = 'filled';
	export let disabled = false;
	export let id: string | undefined = undefined;
	export let loading = false;
	export let tabindex = 0;
	export let wide = false;
	export let grow = false;

	export let element: HTMLAnchorElement | HTMLButtonElement | HTMLElement | null = null;

	let className = '';
	export { className as class };

	const SLOTS = $$props.$$slots;

	onMount(() => {
		if (!element) return;
		element.ariaLabel = element.innerText?.trim();
	});
</script>

<button
	class={`btn ${className}`}
	class:error-outline={color == 'error' && kind == 'outlined'}
	class:primary-outline={color == 'primary' && kind == 'outlined'}
	class:error-filled={color == 'error' && kind == 'filled'}
	class:primary-filled={color == 'primary' && kind == 'filled'}
	class:neutral-outline={color == 'neutral' && kind == 'outlined'}
	class:pointer-events-none={loading}
	class:icon-left={iconAlign == 'left'}
	class:wide
	class:grow
	bind:this={element}
	{disabled}
	on:click
	{id}
	{tabindex}
>
	{#if SLOTS}
		<span class="label text-base-12">
			<slot />
		</span>
	{/if}
	{#if icon}
		<Icon name={loading ? 'spinner' : icon} />
	{:else if loading}
		<Icon name="spinner" />
	{/if}
</button>

<style lang="postcss">
	.btn {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-4) var(--space-6);
		border-radius: var(--radius-m);
		flex-shrink: 0;
		gap: var(--space-2);
		height: var(--size-btn-m);
		min-width: var(--size-btn-m);
		background: transparent;
		transition: background-color var(--transition-fast);
		&:disabled {
			pointer-events: none;
			opacity: 0.6;
		}
		&.wide {
			display: flex;
			width: 100%;
		}
		&.grow {
			flex-grow: 1;
		}
		&.icon-left {
			flex-direction: row-reverse;
		}
	}
	.label {
		display: inline-flex;
		padding: 0 var(--space-2);
	}
	.primary-filled {
		background: var(--clr-theme-pop-element);
		color: var(--clr-theme-pop-on-element);
		&:hover {
			background: var(--clr-theme-pop-element-dim);
		}
		&:active {
			background: var(--clr-theme-pop-element-dark);
		}
	}
	.primary-outline {
		color: var(--clr-theme-pop-outline);
		border: 1px solid var(--clr-theme-pop-outline);
		&:hover {
			color: var(--clr-theme-pop-outline-dim);
			border: 1px solid var(--clr-theme-pop-outline-dim);
		}
		&:active {
			color: var(--clr-theme-pop-outline-dim);
			border: 1px solid var(--clr-theme-pop-outline-dim);
			background: var(--clr-theme-pop-container);
		}
	}
	.neutral-outline {
		color: var(--clr-theme-scale-ntrl-30);
		border: 1px solid var(--clr-theme-container-outline-light);
		&:hover {
			color: var(--clr-theme-scale-ntrl-20);
			border: 1px solid var(--clr-theme-container-outline-pale);
		}
		&:active {
			color: var(--clr-theme-scale-ntrl-20);
			border: 1px solid var(--clr-theme-container-outline-pale);
			background: var(--clr-theme-container-pale);
		}
	}
	.error-filled {
		color: var(--clr-theme-err-on-element);
		background: var(--clr-theme-err-element);
		&:hover {
			background: var(--clr-theme-err-element-dim);
		}
		&:active {
			background: var(--clr-theme-err-element-dark);
		}
	}
	.error-outline {
		color: var(--clr-theme-err-outline);
		border: 1px solid var(--clr-theme-err-outline);
		&:hover {
			color: var(--clr-theme-err-outline-dim);
			border: 1px solid var(--clr-theme-err-outline-dim);
		}
		&:active {
			color: var(--clr-theme-err-outline-dim);
			border: 1px solid var(--clr-theme-err-outline-dim);
			background: var(--clr-theme-err-container);
		}
	}
</style>
