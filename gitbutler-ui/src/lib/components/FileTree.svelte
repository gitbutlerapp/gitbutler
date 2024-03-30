<script context="module" lang="ts">
	let fileTreeId = 0;
</script>

<script lang="ts">
	import TreeListFile from './TreeListFile.svelte';
	import TreeListFolder from './TreeListFolder.svelte';
	import { maybeGetContextStore } from '$lib/utils/context';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import { Ownership } from '$lib/vbranches/ownership';
	import type { TreeNode } from '$lib/vbranches/filetree';
	import type { AnyFile, LocalFile, RemoteFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let expanded = true;
	export let node: TreeNode;
	export let isRoot = false;
	export let showCheckboxes = false;
	export let selectedFiles: Writable<AnyFile[]>;
	export let isUnapplied: boolean;
	export let allowMultiple = false;
	export let readonly = false;
	export let files: LocalFile[] | RemoteFile[];

	const selectedOwnership: Writable<Ownership> | undefined = maybeGetContextStore(Ownership);

	function isNodeChecked(selectedOwnership: Ownership, node: TreeNode): boolean {
		if (node.file) {
			const fileId = node.file.id;
			return node.file.hunks.some((hunk) => selectedOwnership.containsHunk(fileId, hunk.id));
		} else {
			return node.children.every((child) => isNodeChecked(selectedOwnership, child));
		}
	}

	$: isChecked = $selectedOwnership ? isNodeChecked($selectedOwnership, node) : false;

	function isNodeIndeterminate(selectedOwnership: Ownership, node: TreeNode): boolean {
		if (node.file) {
			const fileId = node.file.id;
			const numSelected = node.file.hunks.filter(
				(hunk) => !selectedOwnership.containsHunk(fileId, hunk.id)
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
					{selectedFiles}
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
		selected={$selectedFiles.includes(file)}
		{selectedFiles}
		{readonly}
		showCheckbox={showCheckboxes}
		on:click={(e) => {
			e.stopPropagation();
			const isAlreadySelected = $selectedFiles.includes(file);
			if (isAlreadySelected && e.shiftKey) {
				selectedFiles.update((fileIds) => fileIds.filter((f) => f.id != file.id));
			} else if (isAlreadySelected) {
				$selectedFiles = [];
			} else if (e.shiftKey && allowMultiple) {
				selectedFiles.update((files) => [file, ...files]);
			} else {
				$selectedFiles = [file];
			}
		}}
		on:keydown={(e) => {
			e.preventDefault();
			maybeMoveSelection(e.key, files, selectedFiles);
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
			<div class="flex w-full flex-col overflow-hidden">
				{#each node.children as childNode}
					<svelte:self
						node={childNode}
						expanded={true}
						{showCheckboxes}
						{selectedFiles}
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
		border-left: 1px dashed var(--clr-theme-scale-ntrl-60);
	}
</style>
