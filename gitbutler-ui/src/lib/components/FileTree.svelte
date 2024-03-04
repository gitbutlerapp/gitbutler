<script context="module" lang="ts">
	let fileTreeId = 0;
</script>

<script lang="ts">
	import TreeListFile from './TreeListFile.svelte';
	import TreeListFolder from './TreeListFolder.svelte';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import type { Project } from '$lib/backend/projects';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { TreeNode } from '$lib/vbranches/filetree';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { AnyFile, LocalFile, RemoteFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let project: Project | undefined;
	export let expanded = true;
	export let node: TreeNode;
	export let isRoot = false;
	export let showCheckboxes = false;
	export let selectedOwnership: Writable<Ownership>;
	export let selectedFiles: Writable<AnyFile[]>;
	export let branchId: string;
	export let isUnapplied: boolean;
	export let allowMultiple = false;
	export let readonly = false;
	export let branchController: BranchController;
	export let files: LocalFile[] | RemoteFile[];

	function isNodeChecked(selectedOwnership: Ownership, node: TreeNode): boolean {
		if (node.file) {
			const fileId = node.file.id;
			return node.file.hunks.some((hunk) => selectedOwnership.containsHunk(fileId, hunk.id));
		} else {
			return node.children.every((child) => isNodeChecked(selectedOwnership, child));
		}
	}

	$: isChecked = isNodeChecked($selectedOwnership, node);

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

	$: isIndeterminate = isNodeIndeterminate($selectedOwnership, node);

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
					{selectedOwnership}
					{showCheckboxes}
					{selectedFiles}
					{branchId}
					{isUnapplied}
					{readonly}
					{allowMultiple}
					{branchController}
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
		{branchId}
		{isUnapplied}
		selected={$selectedFiles.includes(file)}
		{selectedOwnership}
		{selectedFiles}
		{readonly}
		{branchController}
		{project}
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
		{selectedOwnership}
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
						{selectedOwnership}
						{showCheckboxes}
						{selectedFiles}
						{branchId}
						{isUnapplied}
						{readonly}
						{allowMultiple}
						{branchController}
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
		padding-left: var(--space-12);
		padding-right: var(--space-6);
	}
	.line {
		width: var(--space-2);
		height: 100%;
		border-left: 1px dashed var(--clr-theme-scale-ntrl-60);
	}
</style>
