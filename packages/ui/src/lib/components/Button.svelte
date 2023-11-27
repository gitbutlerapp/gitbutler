<script lang="ts">
	import { onMount, type ComponentType } from 'svelte';
	import { IconLoading } from '../icons';
	import type iconsJson from '$lib/icons/icons.json';
	import Icon from '$lib/icons/Icon.svelte';

	export let icon: keyof typeof iconsJson | undefined = undefined;
	export let color: 'primary' | 'error' = 'primary';
	export let kind: 'filled' | 'outlined' = 'filled';
	export let disabled = false;
	export let id: string | undefined = undefined;
	export let loading = false;
	export let tabindex = 0;

	let element: HTMLAnchorElement | HTMLButtonElement;

	onMount(() => {
		element.ariaLabel = element.innerText?.trim();
	});
</script>

<button
	class="btn text-base-12"
	class:error-outline={color == 'error' && kind == 'outlined'}
	class:primary-outline={color == 'primary' && kind == 'outlined'}
	class:error-filled={color == 'error' && kind == 'filled'}
	class:primary-filled={color == 'primary' && kind == 'filled'}
	class:pointer-events-none={loading}
	bind:this={element}
	{disabled}
	on:click
	{id}
	{tabindex}
>
	<span>
		<slot />
	</span>
	{#if loading}
		<IconLoading class="animate-spin" />
	{:else if icon}
		<Icon name={icon} />
	{/if}
</button>

<style lang="postcss">
	.btn {
		display: inline-flex;
		padding: var(--space-4) var(--space-6);
		border-radius: var(--radius-m);
		gap: var(--space-2);
		align-items: center;
		height: calc(var(--space-24) + var(--space-2));
		&:disabled {
			opacity: 0.6;
		}
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
