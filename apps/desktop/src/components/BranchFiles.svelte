<script lang="ts">
	import BranchFilesList from './BranchFilesList.svelte';
	import {
		createCommitStore,
		createIntegratedCommitsContextStore,
		createLocalCommitsContextStore,
		createLocalAndRemoteCommitsContextStore
	} from '$lib/commits/contexts';
	import { FileIdSelection } from '$lib/selection/fileIdSelection';
	import { getContext } from '@gitbutler/shared/context';
	import type { DetailedCommit, LocalFile, PatchSeries, RemoteFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	interface Props {
		files: LocalFile[] | RemoteFile[];
		isUnapplied: boolean;
		showCheckboxes?: boolean;
		commitDialogExpanded: Writable<boolean>;
		focusCommitDialog: () => void;
		allowMultiple?: boolean;
		readonly?: boolean;
		branches?: PatchSeries[];
	}

	const {
		files,
		isUnapplied,
		showCheckboxes = false,
		commitDialogExpanded,
		focusCommitDialog,
		allowMultiple = false,
		readonly = false,
		branches
	}: Props = $props();

	createCommitStore(undefined);
	const localCommits = createLocalCommitsContextStore([]);
	const localAndRemoteCommits = createLocalAndRemoteCommitsContextStore([]);
	const integratedCommits = createIntegratedCommitsContextStore([]);

	$effect(() => {
		if (branches) {
			const upstreamPatches: DetailedCommit[] = [];
			const localPatches: DetailedCommit[] = [];

			for (const branch of branches) {
				localPatches.push(...branch.patches);
				upstreamPatches.push(...branch.upstreamPatches);
			}

			localCommits.set(localPatches);
			localAndRemoteCommits.set(upstreamPatches);
			integratedCommits.set(localPatches.filter((p) => p.status === 'integrated'));
		}
	});

	const fileIdSelection = getContext(FileIdSelection);
</script>

<div class="branch-files" role="presentation" onclick={() => fileIdSelection.clear()}>
	{#if files.length > 0}
		<BranchFilesList
			{allowMultiple}
			{readonly}
			{files}
			{showCheckboxes}
			{isUnapplied}
			{commitDialogExpanded}
			{focusCommitDialog}
		/>
	{/if}
</div>

<style lang="postcss">
	.branch-files {
		flex: 1;
	}

	.branch-files:focus {
		outline: none;
	}
</style>
