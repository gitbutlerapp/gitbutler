<script lang="ts">
	import BranchFilesHeader from './BranchFilesHeader.svelte';
	import FileListItem from './FileListItem.svelte';
	import { getContext } from '$lib/utils/context';
	import { selectFilesInList } from '$lib/utils/selectFilesInList';
	import { maybeMoveSelection } from '$lib/utils/selection';
	import { sleep } from '$lib/utils/sleep';
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

	// Appending the files in this manner prevents any major freezing when rendering a long list of files
	// The UI may be slightly sluggish while the files are still rendering in, but its quite a bit of an
	// improvement over being stuck on a frozen icon
	async function pushSortedFiles(files: AnyFile[]) {
		sortedFiles = [];

		for (let filesChunk of chunk(files, 100)) {
			sortedFiles = [...sortedFiles, ...filesChunk];
			await sleep(0);
		}
	}

	$: pushSortedFiles(sortLikeFileTree(files));
</script>

<BranchFilesHeader {files} {showCheckboxes} />
{#each sortedFiles as file (file.id)}
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
