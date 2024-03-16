<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import BranchFilesList from './BranchFilesList.svelte';
	import FileTree from './FileTree.svelte';
	import { filesToFileTree } from '$lib/vbranches/filetree';
	import type { Project } from '$lib/backend/projects';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { LocalFile, RemoteFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let project: Project | undefined;
	export let branchId: string;
	export let files: LocalFile[] | RemoteFile[];
	export let isUnapplied: boolean;
	export let selectedOwnership: Writable<Ownership>;
	export let selectedFiles: Writable<(LocalFile | RemoteFile)[]>;
	export let showCheckboxes = false;

	export let allowMultiple: boolean;
	export let readonly: boolean;

	let selectedListMode: string;
</script>

<div class="branch-files" class:isUnapplied>
	<div class="branch-files__header">
		<BranchFilesHeader {files} {selectedOwnership} {showCheckboxes} bind:selectedListMode />
	</div>
	{#if files.length > 0}
		<div class="files-padding">
			{#if selectedListMode == 'list'}
				<BranchFilesList
					{allowMultiple}
					{readonly}
					{branchId}
					{files}
					{selectedOwnership}
					{selectedFiles}
					{showCheckboxes}
					{isUnapplied}
					{project}
				/>
			{:else}
				<FileTree
					{allowMultiple}
					{readonly}
					node={filesToFileTree(files)}
					{showCheckboxes}
					{branchId}
					isRoot={true}
					{selectedOwnership}
					{selectedFiles}
					{isUnapplied}
					{files}
					{project}
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
		padding-top: var(--space-14);
		padding-bottom: var(--space-12);
		padding-left: var(--space-14);
		padding-right: var(--space-14);
	}
	.files-padding {
		padding-top: 0;
		padding-bottom: var(--space-12);
		padding-left: var(--space-14);
		padding-right: var(--space-14);
	}
</style>
