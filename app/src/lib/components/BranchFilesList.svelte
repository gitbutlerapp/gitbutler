<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import FileListItem from './FileListItem.svelte';
	import { getContext } from '$lib/utils/context';
	import { selectFilesInList } from '$lib/utils/selectFilesInList';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import { getCommitStore } from '$lib/vbranches/contexts';
	import { FileSelection, fileKey } from '$lib/vbranches/fileSelection';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { AnyFile } from '$lib/vbranches/types';

	export let files: AnyFile[];
	export let isUnapplied = false;
	export let showCheckboxes = false;
	export let allowMultiple = false;
	export let readonly = false;

	const fileSelection = getContext(FileSelection);
	const commit = getCommitStore();

	$: sortedFiles = sortLikeFileTree(files);
</script>

<BranchFilesHeader {files} {showCheckboxes} />
{#each sortedFiles as file (file.id)}
	<FileListItem
		{file}
		{readonly}
		{isUnapplied}
		showCheckbox={showCheckboxes}
		selected={$fileSelection.includes(fileKey(file.id, $commit?.id))}
		on:click={(e) => {
			selectFilesInList(e, file, fileSelection, sortedFiles, allowMultiple, $commit);
		}}
		on:keydown={(e) => {
			e.preventDefault();
			maybeMoveSelection(e.key, sortedFiles, fileSelection);
		}}
	/>
{/each}
