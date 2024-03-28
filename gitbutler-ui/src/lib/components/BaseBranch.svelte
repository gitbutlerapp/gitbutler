<script lang="ts">
	import Button from '$lib/components/Button.svelte';
	import CommitCard from '$lib/components/CommitCard.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { projectMergeUpstreamWarningDismissed } from '$lib/config/config';
	import { getContext } from '$lib/utils/context';
	import { tooltip } from '$lib/utils/tooltip';
	import { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, AnyFile } from '$lib/vbranches/types';
	import type { Writable } from 'svelte/store';

	export let base: BaseBranch;
	export let selectedFiles: Writable<AnyFile[]>;

	const branchController = getContext(BranchController);

	const mergeUpstreamWarningDismissed = projectMergeUpstreamWarningDismissed(
		branchController.projectId
	);

	let updateTargetModal: Modal;
	let mergeUpstreamWarningDismissedCheckbox = false;

	$: multiple = base ? base.upstreamCommits.length > 1 || base.upstreamCommits.length == 0 : false;
</script>

<div class="flex flex-col gap-y-4">
	<div class="text-base-13 rounded-sm">
		There {multiple ? 'are' : 'is'}
		{base.upstreamCommits.length} unmerged upstream
		{multiple ? 'commits' : 'commit'}
	</div>
	{#if base.upstreamCommits?.length > 0}
		<div>
			<Button
				color="primary"
				help={'Merges the commits from ' +
					base.branchName +
					' into the base of all applied virtual branches'}
				on:click={() => {
					if ($mergeUpstreamWarningDismissed) {
						branchController.updateBaseBranch();
					} else {
						updateTargetModal.show();
					}
				}}
			>
				Merge into common base
			</Button>
		</div>
		<div class="flex h-full">
			<div class="z-20 flex w-full flex-col gap-2">
				{#each base.upstreamCommits as commit}
					<CommitCard {commit} {selectedFiles} commitUrl={base.commitUrl(commit.id)} />
				{/each}
			</div>
		</div>
		<div
			class="h-px w-full border-none bg-gradient-to-r from-transparent via-light-500 to-transparent dark:via-dark-400"
		/>
	{/if}
	<div>
		<h1
			class="inline-block font-bold text-light-700 dark:text-dark-100"
			use:tooltip={'This is the current base for your virtual branches.'}
		>
			Local
		</h1>
	</div>
	<div class="flex flex-col gap-y-2">
		{#each base.recentCommits as commit}
			<CommitCard {commit} {selectedFiles} commitUrl={base.commitUrl(commit.id)} />
		{/each}
	</div>
</div>

<Modal width="small" bind:this={updateTargetModal} title="Merge Upstream Work">
	<div class="flex flex-col space-y-4">
		<p class="text-blue-600">You are about to merge upstream work from your base branch.</p>
		<p class="font-bold">What will this do?</p>
		<p>
			We will try to merge the work that is upstream into each of your virtual branches, so that
			they are all up to date.
		</p>
		<p>
			Any virtual branches that we can't merge cleanly, we will unapply and mark with a blue dot.
			You can merge these manually later.
		</p>
		<p>Any virtual branches that are fully integrated upstream will be automatically removed.</p>
		<label>
			<input type="checkbox" bind:checked={mergeUpstreamWarningDismissedCheckbox} />
			Don't show this again
		</label>
	</div>
	<svelte:fragment slot="controls" let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button
			color="primary"
			on:click={() => {
				branchController.updateBaseBranch();
				if (mergeUpstreamWarningDismissedCheckbox) {
					mergeUpstreamWarningDismissed.set(true);
				}
				close();
			}}
		>
			Merge Upstream
		</Button>
	</svelte:fragment>
</Modal>
