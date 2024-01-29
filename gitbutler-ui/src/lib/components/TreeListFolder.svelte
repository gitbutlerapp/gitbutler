<script lang="ts">
	import Checkbox from '$lib/components/Checkbox.svelte';
	import { IconFolder } from '$lib/icons';
	import Icon from '$lib/icons/Icon.svelte';
	import type { TreeNode } from '$lib/vbranches/filetree';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Writable } from 'svelte/store';

	export let expanded: boolean;
	export let node: TreeNode;
	export let isChecked = false;
	export let showCheckbox = false;
	export let isIndeterminate = false;
	export let selectedOwnership: Writable<Ownership>;

	function idWithChildren(node: TreeNode): [string, string[]][] {
		if (node.file) {
			return [[node.file.id, node.file.hunks.map((h) => h.id)]];
		}
		return node.children.flatMap(idWithChildren);
	}

	function onSelectionChanged() {
		idWithChildren(node).forEach(([fileId, hunkIds]) => {
			if (isChecked) {
				selectedOwnership.update((ownership) => ownership.removeHunk(fileId, ...hunkIds));
			} else {
				selectedOwnership.update((ownership) => ownership.addHunk(fileId, ...hunkIds));
			}
		});
	}
</script>

<button class="tree-list-folder" class:expanded on:click>
	<div class="chevron-icon" class:chevron-expanded={expanded}>
		<Icon name="chevron-down-small" />
	</div>
	<div class="content-wrapper">
		{#if showCheckbox}
			<Checkbox
				small
				checked={isChecked}
				indeterminate={isIndeterminate}
				on:change={onSelectionChanged}
			/>
		{/if}
		<div class="name-wrapper">
			<IconFolder style="width: var(--space-12)" />
			<span class="name text-base-12">
				{node.name}
			</span>
		</div>
	</div>
</button>

<style lang="postcss">
	.tree-list-folder {
		display: flex;
		align-items: center;
		height: var(--size-btn-m);
		padding: var(--space-4) var(--space-8) var(--space-4) var(--space-4);
		gap: var(--space-6);
		border-radius: var(--radius-s);
		&:hover {
			background: var(--clr-theme-container-pale);

			& .chevron-icon {
				opacity: 0.7;
			}
		}
	}
	.content-wrapper {
		display: flex;
		align-items: center;
		gap: var(--space-10);
	}
	.name-wrapper {
		display: flex;
		align-items: center;
		gap: var(--space-6);
	}
	.name {
		color: var(--clr-theme-scale-ntrl-0);
	}
	.chevron-icon {
		opacity: 0.5;
		transform: rotate(-90deg);
		transition:
			opacity var(--transition-fast),
			transform var(--transition-fast);
	}
	.chevron-expanded {
		transform: rotate(0deg);
	}
</style>
