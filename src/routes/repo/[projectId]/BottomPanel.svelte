<script lang="ts">
	import { slide } from 'svelte/transition';
	import { IconTriangleUp, IconTriangleDown } from '$lib/icons';
	import type { BaseBranch } from '$lib/vbranches';

	export let target: BaseBranch;

	$: console.log(target);
	let shown = false;
</script>

<div class="flex border-t border-light-400 p-2 dark:border-dark-600">
	<div class="ml-4 flex flex-col">
		<div
			role="button"
			tabindex="0"
			class="flex h-[20px] items-center gap-2 text-light-700 hover:text-light-900 dark:text-dark-200 dark:hover:text-dark-100"
			on:click={() => (shown = !shown)}
			on:keypress={() => (shown = !shown)}
		>
			{#if shown}
				<IconTriangleDown />
			{:else}
				<IconTriangleUp />
			{/if}
			<div class="flex flex-row justify-between space-x-2">
				<div class="font-bold uppercase">Common base</div>
			</div>
		</div>
		{#if shown}
			<div class="h-64 py-2" transition:slide={{ duration: 150 }}>
				<div>Hello, my friends, Hello.</div>
			</div>
		{/if}
	</div>
</div>
