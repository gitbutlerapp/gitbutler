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

	$: sortedFiles = sortLikeFileTree(files);
</script>

{#each sortedFiles as file, index (file.id)}
	<FileListItem
		{file}
		{readonly}
		{branchId}
		{isUnapplied}
		{selectedFiles}
		{selectedOwnership}
		{branchController}
		showCheckbox={showCheckboxes}
		selected={$selectedFiles.includes(file)}
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
		on:keydown={(e) => {
			e.preventDefault();
			// Exiting if more one one files are selected or non-arrow keys are pressed
			if ($selectedFiles.length !== 1 || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) {
				return;
			}

			// Update the selected file, given it will be within bounds post update
			if (e.key === 'ArrowUp' && index - 1 >= 0) {
				$selectedFiles = [sortedFiles[index - 1]];
			} else if (e.key === 'ArrowDown' && index + 1 < sortedFiles.length) {
				$selectedFiles = [sortedFiles[index + 1]];
			}

			// TODO: Remove dependency on ID by getting reference to child DOM node
			// Focus on the newly selected file
			const fileElement = document.getElementById('file-' + $selectedFiles[0].id);
			fileElement?.focus();
		}}
	/>
{/each}
