<script lang="ts">
	import { onMount, type ComponentType } from 'svelte';
	import { IconLoading } from '../../icons';

	let classes = '';
	export { classes as class };
	export let color: 'basic' | 'primary' | 'destructive' | 'purple' = 'basic';
	export let kind: 'plain' | 'filled' | 'outlined' = 'filled';
	export let disabled = false;
	export let height: 'basic' | 'small' = 'small';
	export let width: 'basic' | 'full-width' = 'basic';
	export let type: 'button' | 'submit' = 'button';
	export let align: 'left' | 'center' | 'right' = 'center';
	export let icon: ComponentType | undefined = undefined;
	export let loading = false;
	export let tabindex = 0;

	$: filled = kind === 'filled';
	$: outlined = kind === 'outlined';

	let element: HTMLAnchorElement | HTMLButtonElement;

	onMount(() => {
		element.ariaLabel = element.innerText?.trim();
	});
</script>

<button
	class={color + ' ' + classes}
	class:small={height === 'small'}
	class:full-width={width === 'full-width'}
	class:pointer-events-none={loading}
	bind:this={element}
	class:filled
	class:outlined
	{disabled}
	{type}
	class:disabled
	on:click
	class:justify-start={align == 'left'}
	class:justify-center={align == 'center'}
	class:justify-end={align == 'right'}
	class:px-4={!!$$slots.default}
	class:px-2={!$$slots.default}
	{tabindex}
>
	{#if loading}
		{#if icon}
			<IconLoading class="h-4 w-4 animate-spin fill-purple-600 text-light-500 dark:text-dark-500" />
			<slot />
		{:else}
			<div class="items-around absolute flex w-full justify-center">
				<IconLoading
					class="h-4 w-4 animate-spin fill-purple-600 text-light-500 dark:text-dark-500"
				/>
			</div>
			<div class="opacity-0">
				<slot />
			</div>
		{/if}
	{:else}
		<svelte:component this={icon} class="h-5 w-5" />
		<slot />
	{/if}
</button>
