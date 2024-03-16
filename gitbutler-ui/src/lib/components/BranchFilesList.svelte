<script lang="ts">
	import FileListItem from './FileListItem.svelte';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { Project } from '$lib/backend/projects';
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { AnyFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let project: Project | undefined;
	export let branchId: string;
	export let files: AnyFile[];
	export let selectedOwnership: Writable<Ownership>;
	export let isUnapplied = false;
	export let showCheckboxes = false;
	export let selectedFiles: Writable<AnyFile[]>;
	export let allowMultiple = false;
	export let readonly = false;

	$: sortedFiles = sortLikeFileTree(files);
</script>

{#each sortedFiles as file (file.id)}
	<FileListItem
		{file}
		{readonly}
		{branchId}
		{isUnapplied}
		{selectedFiles}
		{selectedOwnership}
		{project}
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
			maybeMoveSelection(e.key, files, selectedFiles);
		}}
	/>
{/each}
