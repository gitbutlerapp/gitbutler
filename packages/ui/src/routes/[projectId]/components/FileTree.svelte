<script context="module" lang="ts">
	let fileTreeId = 0;
</script>

<script lang="ts">
	import { writable, type Writable } from 'svelte/store';
	import type { TreeNode } from '$lib/vbranches/filetree';
	import { Ownership } from '$lib/vbranches/ownership';
	import TreeListFile from './TreeListFile.svelte';
	import TreeListFolder from './TreeListFolder.svelte';

	let className = '';
	export { className as class };
	export let expanded = true;
	export let node: TreeNode;
	export let isRoot = false;
	export let withCheckboxes: boolean = false;
	export let selectedOwnership = writable(Ownership.default());
	export let selectedFileId: Writable<string | undefined>;
	export let branchId: string;
	export let readonly: boolean;

	// function isNodeChecked(selectedOwnership: Ownership, node: TreeNode): boolean {
	// 	if (node.file) {
	// 		const fileId = node.file.id;
	// 		return node.file.hunks.some((hunk) => selectedOwnership.containsHunk(fileId, hunk.id));
	// 	} else {
	// 		return node.children.every((child) => isNodeChecked(selectedOwnership, child));
	// 	}
	// }

	// $: isChecked = isNodeChecked($selectedOwnership, node);

	// function isNodeIndeterminate(selectedOwnership: Ownership, node: TreeNode): boolean {
	// 	if (node.file) {
	// 		const fileId = node.file.id;
	// 		const numSelected = node.file.hunks.filter(
	// 			(hunk) => !selectedOwnership.containsHunk(fileId, hunk.id)
	// 		).length;
	// 		return numSelected !== node.file.hunks.length && numSelected !== 0;
	// 	}
	// 	if (node.children.length === 0) return false;

	// 	const isFirstNodeChecked = isNodeChecked(selectedOwnership, node.children[0]);
	// 	const isFirstNodeIndeterminate = isNodeIndeterminate(selectedOwnership, node.children[0]);
	// 	for (const child of node.children) {
	// 		if (isFirstNodeChecked !== isNodeChecked(selectedOwnership, child)) {
	// 			return true;
	// 		}
	// 		if (isFirstNodeIndeterminate !== isNodeIndeterminate(selectedOwnership, child)) {
	// 			return true;
	// 		}
	// 	}
	// 	return false;
	// }

	// $: isIndeterminate = isNodeIndeterminate($selectedOwnership, node);

	// function idWithChildren(node: TreeNode): [string, string[]][] {
	// 	if (node.file) {
	// 		return [[node.file.id, node.file.hunks.map((h) => h.id)]];
	// 	}
	// 	return node.children.flatMap(idWithChildren);
	// }

	// function onCheckboxChange() {
	// 	idWithChildren(node).forEach(([fileId, hunkIds]) =>
	// 		hunkIds.forEach((hunkId) => {
	// 			if (isChecked) {
	// 				selectedOwnership.update((ownership) => ownership.removeHunk(fileId, hunkId));
	// 			} else {
	// 				selectedOwnership.update((ownership) => ownership.addHunk(fileId, hunkId));
	// 			}
	// 		})
	// 	);
	// }

	function toggle() {
		expanded = !expanded;
	}
</script>

<div class={className}>
	{#if isRoot}
		<!-- Node is a root and should only render children! -->
		<ul id={`fileTree-${fileTreeId++}`}>
			{#each node.children as childNode}
				<li>
					<svelte:self
						node={childNode}
						{selectedOwnership}
						{withCheckboxes}
						{selectedFileId}
						{branchId}
						{readonly}
						on:checked
						on:unchecked
					/>
				</li>
			{/each}
		</ul>
	{:else if node.file}
		<!-- Node is a file -->
		<TreeListFile
			file={node.file}
			{branchId}
			{readonly}
			selected={node.file?.id == $selectedFileId}
			on:click={() => {
				if ($selectedFileId == node.file?.id) $selectedFileId = undefined;
				else $selectedFileId = node.file?.id;
			}}
		/>
	{:else if node.children.length > 0}
		<!-- Node is a folder -->
		<TreeListFolder {node} on:click={toggle} {expanded} />

		<!-- We assume a folder cannot be empty -->
		{#if expanded}
			<div class="nested">
				<div class="line">
					<div class="bg-color-3 inline-block h-full w-px" />
				</div>
				<ul class="w-full overflow-hidden">
					{#each node.children as childNode}
						<li>
							<svelte:self
								node={childNode}
								expanded={true}
								{selectedOwnership}
								{withCheckboxes}
								{selectedFileId}
								{branchId}
								{readonly}
								on:checked
								on:unchecked
							/>
						</li>
					{/each}
				</ul>
			</div>
		{/if}
	{/if}
</div>

<style lang="postcss">
	.nested {
		display: flex;
	}
	.line {
		padding-left: var(--space-12);
		padding-right: var(--space-8);
	}
</style>
