<script lang="ts">
	import type { ComponentType } from 'svelte';

	let className = '';
	export { className as class };
	export let icon: ComponentType;
	export let title: string;
</script>

<button
	on:click|stopPropagation
	class="{className} text-light-600 hover:text-light-800 focus:text-light-800 disabled:cursor-not-allowed disabled:text-light-200 dark:text-dark-400 hover:dark:text-dark-100 focus:dark:text-dark-600 dark:disabled:text-dark-400"
	{title}
>
	<slot />
	<svelte:component this={icon} />
</button>
