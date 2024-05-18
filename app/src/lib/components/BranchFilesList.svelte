<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import FileListItem from './FileListItem.svelte';
	import { getContext } from '$lib/utils/context';
	import { selectFilesInList } from '$lib/utils/selectFilesInList';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import { getCommitStore } from '$lib/vbranches/contexts';
	import { FileIdSelection, fileKey } from '$lib/vbranches/fileIdSelection';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';
	import type { AnyFile } from '$lib/vbranches/types';

	export let files: AnyFile[];
	export let isUnapplied = false;
	export let showCheckboxes = false;
	export let allowMultiple = false;
	export let readonly = false;

	const fileIdSelection = getContext(FileIdSelection);
	const commit = getCommitStore();

	let sortedFiles: AnyFile[] = [];

	function chunk<T>(arr: T[], size: number) {
		return Array.from({ length: Math.ceil(arr.length / size) }, (v, i) =>
			arr.slice(i * size, i * size + size)
		);
	}

	let chunkedFiles: AnyFile[][] = [];
	let displayedFiles: AnyFile[] = [];
	let currentDisplayIndex = 0;

	function setFiles(files: AnyFile[]) {
		chunkedFiles = chunk(sortLikeFileTree(files), 100);
		displayedFiles = chunkedFiles[0] || [];
		currentDisplayIndex = 0;
	}

	// Make sure we display when the file list is reset
	$: setFiles(files);

	export function loadMore() {
		if (currentDisplayIndex + 1 >= chunkedFiles.length) return;

		currentDisplayIndex += 1;
		displayedFiles = [...displayedFiles, ...chunkedFiles[currentDisplayIndex]];
	}
</script>

<BranchFilesHeader {files} {showCheckboxes} />
{#each displayedFiles as file (file.id)}
	<FileListItem
		{file}
		{readonly}
		{isUnapplied}
		showCheckbox={showCheckboxes}
		selected={$fileIdSelection.includes(fileKey(file.id, $commit?.id))}
		on:click={(e) => {
			selectFilesInList(e, file, fileIdSelection, sortedFiles, allowMultiple, $commit);
		}}
		on:keydown={(e) => {
			e.preventDefault();
			maybeMoveSelection(e.key, sortedFiles, fileIdSelection);
		}}
	/>
{/each}
