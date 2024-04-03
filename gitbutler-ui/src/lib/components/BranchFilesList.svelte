<script lang="ts">
	import FileListItem from './FileListItem.svelte';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import { getCommitStore, getSelectedFileIds } from '$lib/vbranches/contexts';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { AnyFile } from '$lib/vbranches/types';

	export let files: AnyFile[];
	export let isUnapplied = false;
	export let showCheckboxes = false;
	export let allowMultiple = false;
	export let readonly = false;

	const fileSelection = getSelectedFileIds();
	const selectedFileIds = $fileSelection.fileIds;
	const commit = getCommitStore();

	$: sortedFiles = sortLikeFileTree(files);
</script>

{#each sortedFiles as file (file.id)}
	<FileListItem
		{file}
		{readonly}
		{isUnapplied}
		showCheckbox={showCheckboxes}
		selected={$selectedFileIds && $fileSelection.has(file.id, $commit?.id)}
		on:click={(e) => {
			const isAlreadySelected = $selectedFileIds && $fileSelection.has(file.id, $commit?.id);
			if (isAlreadySelected && e.shiftKey) {
				$fileSelection.remove(file.id, $commit?.id);
			} else if (isAlreadySelected) {
				$fileSelection.clear();
			} else if (e.shiftKey && allowMultiple) {
				$fileSelection.add(file.id, $commit?.id);
			} else {
				$fileSelection.clear();
				$fileSelection.add(file.id, $commit?.id);
			}
		}}
		on:keydown={(e) => {
			e.preventDefault();
			maybeMoveSelection(e.key, sortedFiles, fileSelection);
		}}
	/>
{/each}
