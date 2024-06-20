<script lang="ts">
	import Checkbox from './Checkbox.svelte';
	import Spacer from './Spacer.svelte';
	import CommitCard from '$lib/commit/CommitCard.svelte';
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { projectMergeUpstreamWarningDismissed } from '$lib/config/config';
	import { showInfo } from '$lib/notifications/toasts';
	import { getContext } from '$lib/utils/context';
	import { tooltip } from '$lib/utils/tooltip';
	import { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch } from '$lib/vbranches/types';

	export let base: BaseBranch;

	const branchController = getContext(BranchController);

	const mergeUpstreamWarningDismissed = projectMergeUpstreamWarningDismissed(
		branchController.projectId
	);

	let updateTargetModal: Modal;
	let mergeUpstreamWarningDismissedCheckbox = false;

	$: multiple = base ? base.upstreamCommits.length > 1 || base.upstreamCommits.length === 0 : false;

	async function updateBaseBranch() {
		let infoText = await branchController.updateBaseBranch();
		if (infoText) {
			showInfo('Stashed conflicting branches', infoText);
		}
	}
</script>

<div class="wrapper">
	<div class="info-text text-base-13">
		There {multiple ? 'are' : 'is'}
		{base.upstreamCommits.length} unmerged upstream
		{multiple ? 'commits' : 'commit'}
	</div>

	{#if base.upstreamCommits?.length > 0}
		<Button
			style="pop"
			kind="solid"
			help={`Merges the commits from ${base.branchName} into the base of all applied virtual branches`}
			on:click={() => {
				if ($mergeUpstreamWarningDismissed) {
					updateBaseBranch();
				} else {
					updateTargetModal.show();
				}
			}}
		>
			Merge into common base
		</Button>
		<div>
			{#each base.upstreamCommits as commit, index}
				<CommitCard
					{commit}
					first={index === 0}
					last={index === base.upstreamCommits.length - 1}
					isUnapplied={true}
					commitUrl={base.commitUrl(commit.id)}
					type="upstream"
				/>
			{/each}
		</div>
		<Spacer margin={2} />
	{/if}

	<div>
		<h1
			class="text-base-13 info-text text-bold"
			use:tooltip={'This is the current base for your virtual branches.'}
		>
			Local
		</h1>
		{#each base.recentCommits as commit, index}
			<CommitCard
				{commit}
				first={index === 0}
				last={index === base.recentCommits.length - 1}
				isUnapplied={true}
				commitUrl={base.commitUrl(commit.id)}
				type="remote"
			/>
		{/each}
	</div>
</div>

<Modal width="small" bind:this={updateTargetModal} title="Merge Upstream Work">
	<div class="modal-content">
		<p class="text-base-body-14">You are about to merge upstream work from your base branch.</p>
	</div>
	<div class="modal-content">
		<h4 class="text-base-body-14 text-semibold">What will this do?</h4>
		<p class="modal__small-text text-base-body-12">
			We will try to merge the work that is upstream into each of your virtual branches, so that
			they are all up to date.
		</p>
		<p class="modal__small-text text-base-body-12">
			Any virtual branches that we can't merge cleanly, we will unapply and mark with a blue dot.
			You can merge these manually later.
		</p>
		<p class="modal__small-text text-base-body-12">
			Any virtual branches that are fully integrated upstream will be automatically removed.
		</p>
	</div>

	<label class="modal__dont-show-again" for="dont-show-again">
		<Checkbox name="dont-show-again" bind:checked={mergeUpstreamWarningDismissedCheckbox} />
		<span class="text-base-12">Don't show this again</span>
	</label>

	{#snippet controls(close)}
		<Button style="ghost" outline on:click={close}>Cancel</Button>
		<Button
			style="pop"
			kind="solid"
			on:click={() => {
				updateBaseBranch();
				if (mergeUpstreamWarningDismissedCheckbox) {
					mergeUpstreamWarningDismissed.set(true);
				}
				close();
			}}
		>
			Merge Upstream
		</Button>
	{/snippet}
</Modal>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.info-text {
		opacity: 0.5;
	}

	.modal-content {
		display: flex;
		flex-direction: column;
		gap: 10px;
		margin-bottom: 20px;

		&:last-child {
			margin-bottom: 0;
		}
	}

	.modal__small-text {
		opacity: 0.6;
	}

	.modal__dont-show-again {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 14px;
		background-color: var(--clr-bg-2);
		border-radius: var(--radius-m);
		margin-bottom: 6px;
	}
</style>
