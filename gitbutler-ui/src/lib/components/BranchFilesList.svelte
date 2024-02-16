<script lang="ts">
	import FileListItem from './FileListItem.svelte';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { AnyFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let branchId: string;
	export let files: AnyFile[];
	export let selectedOwnership: Writable<Ownership>;
	export let isUnapplied = false;
	export let showCheckboxes = false;
	export let selectedFiles: Writable<AnyFile[]>;
	export let allowMultiple = false;
	export let readonly = false;
	export let branchController: BranchController;
</script>

{#each sortLikeFileTree(files) as file (file.id)}
	<FileListItem
		{file}
		{readonly}
		{branchId}
		{isUnapplied}
		{selectedFiles}
		{selectedOwnership}
		{branchController}
		showCheckbox={showCheckboxes}
		on:click={(e) => {
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
		selected={$selectedFiles.includes(file)}
	/>
{/each}
