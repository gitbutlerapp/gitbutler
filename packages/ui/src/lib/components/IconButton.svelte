<script lang="ts">
	import type { ComponentType } from 'svelte';

	let className = '';
	export { className as class };
	export let icon: ComponentType | undefined = undefined;
	export let title = '';
</script>

<button
	class="{className} text-color-4 hover:text-color-1 outline-blue-200 focus:text-light-800 focus:outline-2 disabled:cursor-not-allowed disabled:text-light-200 dark:text-dark-400 hover:dark:text-dark-100 focus:dark:text-dark-600 dark:disabled:text-dark-400"
	{title}
	on:click
>
	<slot />
	<svelte:component this={icon} class="h-3 w-3" />
</button>
