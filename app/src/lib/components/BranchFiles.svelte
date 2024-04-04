<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import { getContext } from '$lib/utils/context';
	import { createCommitStore } from '$lib/vbranches/contexts';
	import { FileSelection } from '$lib/vbranches/fileSelection';
	import type { LocalFile, RemoteFile } from '$lib/vbranches/types';

	export let files: LocalFile[] | RemoteFile[];
	export let isUnapplied: boolean;
	export let showCheckboxes = false;

	export let allowMultiple = false;
	export let readonly = false;

	createCommitStore(undefined);
	const fileSelection = getContext(FileSelection);

	function unselectAllFiles() {
		fileSelection.clear();
	}
</script>

<div
	class="branch-files"
	role="listbox"
	tabindex="-1"
	on:keydown={(e) => {
		if (e.key === 'Escape') {
			unselectAllFiles();
		}
	}}
	on:click={unselectAllFiles}
>
	{#if files.length > 0}
		<BranchFilesList {allowMultiple} {readonly} {files} {showCheckboxes} {isUnapplied} />
	{/if}
</div>

<style lang="postcss">
	.branch-files {
		flex: 1;
		display: flex;
		flex-direction: column;
		background: var(--clr-theme-container-light);
		border-radius: var(--radius-m) var(--radius-m) 0 0;
		padding: 0 var(--size-14) var(--size-14);
	}
</style>
