<script lang="ts">
	import StackingStatusIcon from './StackingStatusIcon.svelte';
	import { Project } from '$lib/backend/projects';
	import { projectShowStackingCardDetails } from '$lib/config/config';
	import Link from '$lib/shared/Link.svelte';
	import Spacer from '$lib/shared/Spacer.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { error } from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

	const project = getContext(Project);
	const branchController = getContext(BranchController);
	const branch = getContextStore(VirtualBranch);

	const showStackingCardDetails = projectShowStackingCardDetails(project.id);

	let showDetails = $state($showStackingCardDetails);
	let loading = $state(false);

	let createRefModal: Modal;
	let createRefName: string | undefined = $state();

	function closeStackingCard() {
		showStackingCardDetails.set(false);
		showDetails = false;
	}

	function addSeries() {
		if (!createRefName) {
			error('No branch name provided');
			createRefModal.close();
			return;
		}
		loading = true;
		try {
			branchController.createPatchSeries($branch.id, createRefName);
			createRefModal.close();
		} finally {
			loading = false;
		}
	}
</script>

<section class="card">
	{#if showDetails}
		<button tabindex="0" class="card__close" onclick={closeStackingCard}>
			<Icon name="cross-small" />
		</button>
		<div class="card__body">
			<h2 class="text-16 text-bold">New branch stacking</h2>
			<p class="text-12 card__description">
				Allows you to add a branch that depends on previous branches. This helps you create smaller
				PRs that are reviewed and merged in order.
				<Link href="https://docs.gitbutler.com/stacking" target="_blank">Read more</Link>
			</p>
		</div>
		<Spacer />
	{/if}
	<section class="card__action" class:showDetails={!showDetails}>
		<StackingStatusIcon icon="plus-small" gap={true} />
		<Button grow style="neutral" {loading} onclick={() => createRefModal.show()}
			>Add a branch to the stack</Button
		>
	</section>
</section>

<Modal
	bind:this={createRefModal}
	title="Add branch to the stack"
	width="small"
	onSubmit={addSeries}
>
	{#snippet children()}
		<TextBox placeholder="New branch name" id="newRemoteName" bind:value={createRefName} focus />
	{/snippet}
	{#snippet controls(close)}
		<Button style="pop" kind="solid">Ok</Button>
		<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
	{/snippet}
</Modal>

<style>
	.card {
		position: relative;
		display: flex;
		flex-direction: column;
	}

	.card__body {
		padding: 16px 16px 0 16px;
	}

	.card__close {
		position: absolute;
		top: 8px;
		right: 8px;

		color: var(--clr-scale-ntrl-60);
	}

	.card__description {
		color: var(--clr-scale-ntrl-50);
		line-height: 18px;
	}

	.card__action {
		width: 100%;
		display: flex;
		justify-content: around;
		align-items: flex-start;
		padding: 0 13px;
		gap: 1rem;

		&.showDetails {
			margin-top: 16px;
		}
	}
</style>
