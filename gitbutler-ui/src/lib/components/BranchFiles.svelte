<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import BranchFilesList from './BranchFilesList.svelte';
	import FileTree from './FileTree.svelte';
	import { filesToFileTree } from '$lib/vbranches/filetree';
	import type { LocalFile, RemoteFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let files: LocalFile[] | RemoteFile[];
	export let isUnapplied: boolean;
	export let selectedFiles: Writable<(LocalFile | RemoteFile)[]>;
	export let showCheckboxes = false;

	export let allowMultiple = false;
	export let readonly = false;

	let selectedListMode: string;
</script>

<div class="branch-files" class:isUnapplied>
	<div class="branch-files__header">
		<BranchFilesHeader {files} {showCheckboxes} bind:selectedListMode />
	</div>
	{#if files.length > 0}
		<div class="files-padding">
			{#if selectedListMode == 'list'}
				<BranchFilesList
					{allowMultiple}
					{readonly}
					{files}
					{selectedFiles}
					{showCheckboxes}
					{isUnapplied}
				/>
			{:else}
				<FileTree
					{allowMultiple}
					{readonly}
					node={filesToFileTree(files)}
					{showCheckboxes}
					isRoot={true}
					{selectedFiles}
					{isUnapplied}
					{files}
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
		padding-top: var(--size-14);
		padding-bottom: var(--size-12);
		padding-left: var(--size-14);
		padding-right: var(--size-14);
	}
	.files-padding {
		padding-top: 0;
		padding-bottom: var(--size-12);
		padding-left: var(--size-14);
		padding-right: var(--size-14);
	}
</style>
