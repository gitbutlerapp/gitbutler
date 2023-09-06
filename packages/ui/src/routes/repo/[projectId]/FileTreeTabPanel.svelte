<script lang="ts">
	import { filesToFileTree } from '$lib/vbranches/filetree';
	import type { File } from '$lib/vbranches/types';
	import FileTree from './FileTree.svelte';

	export let files: File[];
</script>

<FileTree node={filesToFileTree(files)} isRoot={true} class="p-2" />
