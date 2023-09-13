<script lang="ts">
	import { filesToFileTree } from '$lib/vbranches/filetree';
	import type { File } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';
	import FileTree from './FileTree.svelte';
	import type { Ownership } from '$lib/vbranches/ownership';

	export let files: File[];
	export let withCheckboxes: boolean;
	export let selectedOwnership: Writable<Ownership>;
</script>

<FileTree
	node={filesToFileTree(files)}
	isRoot={true}
	class="p-2"
	{selectedOwnership}
	{withCheckboxes}
/>
