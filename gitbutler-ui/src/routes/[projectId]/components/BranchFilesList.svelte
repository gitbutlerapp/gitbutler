<script lang="ts">
	import type { Ownership } from '$lib/vbranches/ownership';
	import type { Branch } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';
	import FileListItem from './FileListItem.svelte';
	import { sortLikeFileTree } from '$lib/vbranches/filetree';

	export let branch: Branch;
	export let selectedOwnership: Writable<Ownership>;
	export let readonly = false;
	export let showCheckboxes = false;
	export let selectedFileId: Writable<string | undefined>;
</script>

{#each sortLikeFileTree(branch.files) as file (file.id)}
	<FileListItem
		{file}
		{readonly}
		branchId={branch.id}
		{selectedOwnership}
		showCheckbox={showCheckboxes}
		on:click={() => {
			if ($selectedFileId == file.id) $selectedFileId = undefined;
			else $selectedFileId = file.id;
		}}
		selected={file.id == $selectedFileId}
	/>
{/each}
