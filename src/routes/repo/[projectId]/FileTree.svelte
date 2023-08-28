<script context="module" lang="ts">
	let fileTreeId = 0;
</script>

<script lang="ts">
	import IconChevronDownSmall from '$lib/icons/IconChevronDownSmall.svelte';
	import IconChevronRightSmall from '$lib/icons/IconChevronRightSmall.svelte';
	import IconFile from '$lib/icons/IconFile.svelte';
	import IconFolder from '$lib/icons/IconFolder.svelte';
	import type { TreeNode } from '$lib/vbranches/filetree';

	export let expanded = true;
	export let node: TreeNode;
	export let isRoot = false;

	function toggle() {
		expanded = !expanded;
	}
</script>

{#if isRoot}
	<!-- Node is a root and should only render children -->
	<ul id={`fileTree-${fileTreeId++}`}>
		{#each node.children as childNode}
			<li>
				<svelte:self node={childNode} />
			</li>
		{/each}
	</ul>
{:else if node.file}
	{@const { added, removed } = node.file.getAddedAndRemoved()}
	<!-- Node is a file -->
	<div class="flex w-full items-center gap-x-2 py-0 text-left">
		<div class="w-2 shrink-0" />
		<div class="w-4 shrink-0 text-center">
			<IconFile class="h-4 w-4" />
		</div>
		<div class="flex-grow truncate">
			{node.name}
		</div>
		<div class="flex gap-1 font-mono text-xs font-bold">
			<span class="font-mono text-green-500">
				+{added}
			</span>
			<span class="font-mono text-red-500">
				-{removed}
			</span>
		</div>
	</div>
{:else if node.children.length > 0}
	<!-- Node is a folder -->
	<button class="flex w-full items-center gap-x-2 py-0 text-left" class:expanded on:click={toggle}>
		<div class="w-3 shrink-0 text-center">
			{#if expanded}
				<IconChevronDownSmall class="scale-90 text-light-600 dark:text-dark-200" />
			{:else}
				<IconChevronRightSmall class="scale-90 text-light-600 dark:text-dark-200" />
			{/if}
		</div>
		<div class="w-4 shrink-0 text-center">
			<IconFolder class="h-4 w-4 text-blue-400" />
		</div>
		<div class="flex-grow truncate">
			{node.name}
		</div>
	</button>
	<!-- We assume a folder cannot be empty -->
	{#if expanded}
		<div class="flex">
			<div class="flex">
				<div class="w-3 shrink-0 text-center">
					<div class="inline-block h-full w-px bg-light-200 dark:bg-dark-400" />
				</div>
			</div>
			<ul class="w-full overflow-hidden">
				{#each node.children as childNode}
					<li>
						<svelte:self node={childNode} expanded={true} />
					</li>
				{/each}
			</ul>
		</div>
	{/if}
{/if}
