<script lang="ts">
	import { filesToFileTree } from '$lib/vbranches/filetree';
	import type { File } from '$lib/vbranches/types';
	import FileTree from './FileTree.svelte';
	import type { Writable } from 'svelte/store';

	export let files: File[];
	export let withCheckboxes: boolean;
	export let selectedFileIds: Writable<string[]>;

	function handleChecked(event: CustomEvent<{ fileId: string }>) {
		$selectedFileIds = [...$selectedFileIds, event.detail.fileId];
	}

	function handleUnchecked(event: CustomEvent<{ fileId: string }>) {
		$selectedFileIds = $selectedFileIds.filter((id) => id !== event.detail.fileId);
	}
</script>

<FileTree
	node={filesToFileTree(files)}
	isRoot={true}
	class="p-2"
	selectedFileIds={$selectedFileIds}
	on:checked={handleChecked}
	on:unchecked={handleUnchecked}
	{withCheckboxes}
/>
