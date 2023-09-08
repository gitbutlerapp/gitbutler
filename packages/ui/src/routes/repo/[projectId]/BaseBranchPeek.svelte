<script lang="ts">
	import { Button, Modal, Tooltip } from '$lib/components';
	import type { BaseBranch } from '$lib/vbranches/types';
	import CommitCard from './CommitCard.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { projectMergeUpstreamWarningDismissed } from '$lib/config/config';

	export let base: BaseBranch;
	export let branchController: BranchController;
	const mergeUpstreamWarningDismissed = projectMergeUpstreamWarningDismissed(
		branchController.projectId
	);

	let updateTargetModal: Modal;
	let viewport: HTMLElement;
	let contents: HTMLElement;
	let mergeUpstreamWarningDismissedCheckbox = false;

	$: multiple = base ? base.upstreamCommits.length > 1 || base.upstreamCommits.length == 0 : false;
</script>

<div class="relative h-full max-h-full">
	<div
		bind:this={viewport}
		class="hide-native-scrollbar dark:bg-dark-900 flex max-h-full flex-grow flex-col overflow-y-scroll overscroll-none"
	>
		<div bind:this={contents} class="flex flex-col gap-y-4 p-4">
			<h1 class="text-light-700 dark:text-dark-100 font-bold">Upstream</h1>
			<div class="text-light-700 dark:text-dark-200 rounded-sm text-sm">
				There {multiple ? 'are' : 'is'}
				{base.upstreamCommits.length} unmerged upstream
				{multiple ? 'commits' : 'commit'}
			</div>
			{#if base.upstreamCommits?.length > 0}
				<div>
					<Tooltip
						label={'Merges the commits from ' +
							base.branchName +
							' into the base of all applied virtual branches'}
					>
						<Button
							width="full-width"
							height="small"
							color="purple"
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
					</Tooltip>
				</div>
				<div class="flex h-full">
					<div class="z-20 flex w-full flex-col gap-2">
						{#each base.upstreamCommits as commit}
							<CommitCard
								{commit}
								url={base.commitUrl(commit.id)}
								projectId={branchController.projectId}
							/>
						{/each}
					</div>
				</div>
				<div
					class="via-light-500 dark:via-dark-400 h-px w-full border-none bg-gradient-to-r from-transparent to-transparent"
				/>
			{/if}
			<Tooltip label="This is the current base for your virtual branches.">
				<h1 class="text-light-700 dark:text-dark-100 font-bold">Local</h1>
			</Tooltip>
			<div class="flex flex-col gap-y-2">
				{#each base.recentCommits as commit}
					<CommitCard
						url={base.commitUrl(commit.id)}
						{commit}
						projectId={branchController.projectId}
					/>
				{/each}
			</div>
		</div>
	</div>
	<Scrollbar {viewport} {contents} width="0.5rem" />
</div>
<!-- Confirm target update modal -->

<Modal width="small" bind:this={updateTargetModal}>
	<svelte:fragment slot="title">Merge Upstream Work</svelte:fragment>
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
		<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
		<Button
			height="small"
			color="purple"
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
