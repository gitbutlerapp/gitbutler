<script lang="ts">
	import { IconFolder } from '$lib/icons';
	import Icon from '$lib/icons/Icon.svelte';
	import type { TreeNode } from '$lib/vbranches/filetree';

	export let expanded: boolean;
	export let node: TreeNode;
</script>

<button class="tree-list-folder" class:expanded on:click>
	{#if expanded}
		<Icon name="chevron-down-small" />
	{:else}
		<Icon name="chevron-right-small" />
	{/if}
	<IconFolder class="h-4 w-4 scale-75 text-blue-400" />
	<span class="name text-base-12">
		{node.name}
	</span>
	<!-- {#if withCheckboxes}
		<Checkbox checked={isChecked} indeterminate={isIndeterminate} on:change={onCheckboxChange} />
	{/if} -->
</button>

<style lang="postcss">
	.tree-list-folder {
		display: flex;
		align-items: center;
		padding: var(--space-4);
		gap: var(--space-6);
		border-radius: var(--radius-s);
		&:hover {
			background: var(--clr-theme-container-pale);
		}
	}
	.name {
		color: var(--clr-theme-scale-ntrl-0);
	}
</style>
