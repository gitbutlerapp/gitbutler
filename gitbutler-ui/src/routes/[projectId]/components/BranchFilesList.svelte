<script lang="ts">
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Branch, File } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';
	import FileListItem from './FileListItem.svelte';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';

	export let branch: Branch;
	export let selectedOwnership: Writable<Ownership>;
	export let readonly = false;
	export let showCheckboxes = false;
	export let selectedFiles: Writable<File[]>;
</script>

{#each sortLikeFileTree(branch.files) as file (file.id)}
	<FileListItem
		{file}
		{readonly}
		branchId={branch.id}
		{selectedOwnership}
		showCheckbox={showCheckboxes}
		{selectedFiles}
		on:click={(e) => {
			const isAlreadySelected = $selectedFiles.includes(file);
			if (isAlreadySelected && e.shiftKey) {
				selectedFiles.update((fileIds) => fileIds.filter((f) => f.id != file.id));
			} else if (isAlreadySelected) {
				$selectedFiles = [];
			} else if (e.shiftKey) {
				selectedFiles.update((files) => [file, ...files]);
			} else {
				$selectedFiles = [file];
			}
		}}
		selected={$selectedFiles.includes(file)}
	/>
{/each}
