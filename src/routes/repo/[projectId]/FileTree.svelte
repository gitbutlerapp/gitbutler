<script lang="ts">
	import IconChevronDownSmall from '$lib/icons/IconChevronDownSmall.svelte';
	import IconChevronRightSmall from '$lib/icons/IconChevronRightSmall.svelte';
	import IconFile from '$lib/icons/IconFile.svelte';
	import IconFolder from '$lib/icons/IconFolder.svelte';
	import type { TreeNode } from '$lib/vbranches/filetree';
	import type { File } from '$lib/vbranches/types';

	export let expanded = false;
	export let name: string;
	export let nodes: TreeNode[];
	export let file: File | undefined = undefined;

	function toggle() {
		expanded = !expanded;
	}
</script>

<button class="flex w-full items-center gap-x-2 py-0 text-left" class:expanded on:click={toggle}>
	{#if !file}
		<div class="w-3 shrink-0 text-center">
			{#if expanded}
				<IconChevronDownSmall class="scale-90 text-light-600 dark:text-dark-200" />
			{:else}
				<IconChevronRightSmall class="scale-90 text-light-600 dark:text-dark-200" />
			{/if}
		</div>
	{:else}
		<div class="w-2 shrink-0" />
	{/if}
	<div class="w-4 shrink-0 text-center">
		{#if file}
			<IconFile class="h-4 w-4" />
		{:else}
			<IconFolder class="h-4 w-4 text-blue-400" />
		{/if}
	</div>
	<div class="flex-grow truncate">
		{name}
	</div>
</button>
{#if !file && expanded}
	<div class="flex">
		<div class="w-3 shrink-0 text-center">
			<div class="inline-block h-full w-px bg-light-200 dark:bg-dark-400" />
		</div>
		{#if expanded && nodes}
			<ul class="w-full overflow-hidden">
				{#each nodes as node}
					<li>
						<svelte:self name={node.name} nodes={node.children} expanded={true} file={node.file} />
					</li>
				{/each}
			</ul>
		{/if}
	</div>
{/if}
