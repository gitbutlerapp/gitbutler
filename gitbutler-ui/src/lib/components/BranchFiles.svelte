<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import BranchFilesList from './BranchFilesList.svelte';
	import FileTree from './FileTree.svelte';
	import { filesToFileTree } from '$lib/vbranches/filetree';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Branch, LocalFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let branch: Branch;
	export let isUnapplied: boolean;
	export let selectedOwnership: Writable<Ownership>;
	export let selectedFiles: Writable<LocalFile[]>;
	export let showCheckboxes = false;
	export let branchController: BranchController;

	let selectedListMode: string;
</script>

{#if branch.active && branch.conflicted}
	<div class="mb-2 bg-red-500 p-2 font-bold text-white">
		{#if branch.files.some((f) => f.conflicted)}
			This virtual branch conflicts with upstream changes. Please resolve all conflicts and commit
			before you can continue.
		{:else}
			Please commit your resolved conflicts to continue.
		{/if}
	</div>
{/if}

<div class="branch-files" class:isUnapplied>
	<div class="branch-files__header">
		<BranchFilesHeader
			files={branch.files}
			{selectedOwnership}
			{showCheckboxes}
			bind:selectedListMode
		/>
	</div>
	{#if branch.files.length > 0}
		<div class="files-padding">
			{#if selectedListMode == 'list'}
				<BranchFilesList
					allowMultiple
					branchId={branch.id}
					files={branch.files}
					{selectedOwnership}
					{selectedFiles}
					{showCheckboxes}
					{isUnapplied}
					{branchController}
				/>
			{:else}
				<FileTree
					allowMultiple
					node={filesToFileTree(branch.files)}
					{showCheckboxes}
					branchId={branch.id}
					isRoot={true}
					{selectedOwnership}
					{selectedFiles}
					{isUnapplied}
				/>
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.branch-files {
		flex: 1;
		background: var(--clr-theme-container-light);
		border-radius: var(--radius-m) var(--radius-m) 0 0;

		&.isUnapplied {
			border-radius: var(--radius-m);
		}
	}
	.branch-files__header {
		padding-top: var(--space-12);
		padding-bottom: var(--space-12);
		padding-left: var(--space-20);
		padding-right: var(--space-12);
	}
	.files-padding {
		padding-top: 0;
		padding-bottom: var(--space-12);
		padding-left: var(--space-12);
		padding-right: var(--space-12);
	}
</style>
