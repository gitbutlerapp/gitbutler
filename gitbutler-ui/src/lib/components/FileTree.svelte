<script context="module" lang="ts">
	let fileTreeId = 0;
</script>

<script lang="ts">
	import TreeListFile from './TreeListFile.svelte';
	import TreeListFolder from './TreeListFolder.svelte';
	import { maybeGetContextStore } from '$lib/utils/context';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import { getSelectedFileIds } from '$lib/vbranches/contexts';
	import { Ownership } from '$lib/vbranches/ownership';
	import type { TreeNode } from '$lib/vbranches/filetree';
	import type { LocalFile, RemoteFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let expanded = true;
	export let node: TreeNode;
	export let isRoot = false;
	export let showCheckboxes = false;
	export let isUnapplied: boolean;
	export let allowMultiple = false;
	export let readonly = false;
	export let files: LocalFile[] | RemoteFile[];

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);
	const fileSelection = getSelectedFileIds();
	const selectedFileIds = $fileSelection.fileIds;

	function isNodeChecked(selectedOwnership: Ownership, node: TreeNode): boolean {
		if (node.file) {
			const fileId = node.file.id;
			return node.file.hunks.some((hunk) => selectedOwnership.contains(fileId, hunk.id));
		} else {
			return node.children.every((child) => isNodeChecked(selectedOwnership, child));
		}
	}

	$: isChecked = $selectedOwnership ? isNodeChecked($selectedOwnership, node) : false;

	function isNodeIndeterminate(selectedOwnership: Ownership, node: TreeNode): boolean {
		if (node.file) {
			const fileId = node.file.id;
			const numSelected = node.file.hunks.filter(
				(hunk) => !selectedOwnership.contains(fileId, hunk.id)
			).length;
			return numSelected !== node.file.hunks.length && numSelected !== 0;
		}
		if (node.children.length === 0) return false;

		const isFirstNodeIndeterminate = isNodeIndeterminate(selectedOwnership, node.children[0]);
		if (node.children.length == 1 && isFirstNodeIndeterminate) return true;
		const isFirstNodeChecked = isNodeChecked(selectedOwnership, node.children[0]);
		for (const child of node.children) {
			if (isFirstNodeChecked !== isNodeChecked(selectedOwnership, child)) {
				return true;
			}
			if (isFirstNodeIndeterminate !== isNodeIndeterminate(selectedOwnership, child)) {
				return true;
			}
		}
		return false;
	}

	$: isIndeterminate = $selectedOwnership ? isNodeIndeterminate($selectedOwnership, node) : false;

	function toggle() {
		expanded = !expanded;
	}
</script>

{#if isRoot}
	<!-- Node is a root and should only render children! -->
	<ul id={`fileTree-${fileTreeId++}`}>
		{#each node.children as childNode}
			<li>
				<svelte:self
					node={childNode}
					{showCheckboxes}
					{isUnapplied}
					{readonly}
					{allowMultiple}
					{files}
					on:checked
					on:unchecked
				/>
			</li>
		{/each}
	</ul>
{:else if node.file}
	{@const file = node.file}
	<!-- Node is a file -->
	<TreeListFile
		file={node.file}
		{isUnapplied}
		selected={$selectedFileIds.includes(file.id)}
		{readonly}
		showCheckbox={showCheckboxes}
		on:click={(e) => {
			e.stopPropagation();
			const isAlreadySelected = $fileSelection.has(file.id);
			if (isAlreadySelected && e.shiftKey) {
				$fileSelection.remove(file.id);
			} else if (isAlreadySelected) {
				$fileSelection.clear();
			} else if (e.shiftKey && allowMultiple) {
				$fileSelection.add(file.id);
			} else {
				$fileSelection.clear();
				$fileSelection.add(file.id);
			}
		}}
		on:keydown={(e) => {
			e.preventDefault();
			maybeMoveSelection(e.key, files, fileSelection);
		}}
	/>
{:else if node.children.length > 0}
	<!-- Node is a folder -->
	<TreeListFolder
		showCheckbox={showCheckboxes}
		{isIndeterminate}
		{isChecked}
		{node}
		on:mousedown={toggle}
		{expanded}
	/>

	<!-- We assume a folder cannot be empty -->
	{#if expanded}
		<div class="nested">
			<div class="line-wrapper">
				<div class="line" />
			</div>
			<div class="files-list">
				{#each node.children as childNode}
					<svelte:self
						node={childNode}
						expanded={true}
						{showCheckboxes}
						{isUnapplied}
						{readonly}
						{allowMultiple}
						{files}
						on:checked
						on:unchecked
					/>
				{/each}
			</div>
		</div>
	{/if}
{/if}

<style lang="postcss">
	.nested {
		display: flex;
	}
	.line-wrapper {
		position: relative;
		padding-left: var(--size-12);
		padding-right: var(--size-6);
	}
	.line {
		width: var(--size-2);
		height: 100%;
		border-left: 1px dashed var(--clr-scale-ntrl-60);
	}
	.files-list {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		width: 100%;
	}
</style>
