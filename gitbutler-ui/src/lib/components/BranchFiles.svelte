<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import { createCommitStore } from '$lib/vbranches/contexts';
	import type { LocalFile, RemoteFile } from '$lib/vbranches/types';

	export let files: LocalFile[] | RemoteFile[];
	export let isUnapplied: boolean;
	export let showCheckboxes = false;

	export let allowMultiple = false;
	export let readonly = false;

	createCommitStore(undefined);
</script>

<div class="branch-files" class:isUnapplied>
	{#if files.length > 0}
		<div class="files-padding">
			<BranchFilesList {allowMultiple} {readonly} {files} {showCheckboxes} {isUnapplied} />
		</div>
	{/if}
</div>

<style lang="postcss">
	.branch-files {
		flex: 1;
		background: var(--clr-container-light);
		border-radius: var(--radius-m) var(--radius-m) 0 0;

		&.isUnapplied {
			border-radius: var(--radius-m);
		}
	}
	.files-padding {
		padding-top: 0;
		padding-bottom: var(--size-12);
		padding-left: var(--size-14);
		padding-right: var(--size-14);
	}
</style>
