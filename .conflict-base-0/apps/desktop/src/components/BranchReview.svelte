<script lang="ts">
	import CanPublishReviewPlugin from '$components/CanPublishReviewPlugin.svelte';
	import PullRequestCard from '$components/PullRequestCard.svelte';
	import ReviewCreation from '$components/ReviewCreation.svelte';
	import ReviewCreationControls from '$components/ReviewCreationControls.svelte';
	import StackedPullRequestCard from '$components/StackedPullRequestCard.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import type { Snippet } from 'svelte';

	// TODO: This and the SeriesHeader should have a wholistic refactor to
	// reduce the complexity of the forge related functionality.

	type Props = {
		branchStatus?: Snippet;
		projectId: string;
		stackId?: string;
		branchName: string;
		prNumber?: number;
		reviewId?: string;
	};

	const { branchStatus, projectId, stackId, branchName, prNumber, reviewId }: Props = $props();

	let canPublishReviewPlugin = $state<ReturnType<typeof CanPublishReviewPlugin>>();

	const forge = inject(DEFAULT_FORGE_FACTORY);
	const prService = $derived(forge.current.prService);
	const reviewUnit = $derived(prService?.unit.abbr);

	const canPublishPR = $derived(!!canPublishReviewPlugin?.imports.canPublishPR);

	let modal = $state<Modal>();
	let confirmCreatePrModal = $state<ReturnType<typeof Modal>>();
	let reviewCreation = $state<ReturnType<typeof ReviewCreation>>();
</script>

<CanPublishReviewPlugin
	bind:this={canPublishReviewPlugin}
	{projectId}
	{stackId}
	{branchName}
	{prNumber}
	{reviewId}
/>

{#if stackId}
	<Modal
		width="small"
		type="warning"
		title="Create Pull Request"
		bind:this={confirmCreatePrModal}
		onSubmit={() => {
			modal?.show();
		}}
	>
		<p class="text-13 text-body helper-text">
			It's strongly recommended to create pull requests starting with the branch at the base of the
			stack.
			<br />
			Do you still want to create this pull request?
		</p>
		{#snippet controls(close)}
			<Button kind="outline" onclick={close}>Cancel</Button>
			<Button style="warning" type="submit">Create Pull Request</Button>
		{/snippet}
	</Modal>

	<Modal bind:this={modal} title="Submit changes for review">
		<ReviewCreation
			bind:this={reviewCreation}
			{projectId}
			{stackId}
			{branchName}
			onClose={() => modal?.close()}
		/>

		{#snippet controls(close)}
			<ReviewCreationControls
				isSubmitting={!!reviewCreation?.imports.isLoading}
				{canPublishPR}
				{reviewUnit}
				onCancel={close}
				onSubmit={async () => {
					await reviewCreation?.createReview();
				}}
			/>
		{/snippet}
	</Modal>
{/if}

{#if prNumber || branchStatus}
	<div class="branch-action">
		{#if prNumber}
			<div class="status-cards">
				{#if prNumber && stackId}
					<StackedPullRequestCard {projectId} {stackId} {branchName} {prNumber} poll />
				{:else if prNumber}
					<PullRequestCard {branchName} {prNumber} poll />
				{/if}
			</div>
		{/if}

		{#if branchStatus}
			{@render branchStatus()}
		{/if}
	</div>
{/if}

<style lang="postcss">
	.branch-action {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 14px;
	}

	.status-cards {
		display: flex;
		flex-direction: column;
		gap: 8px;

		& :global(.review-card) {
			display: flex;
			position: relative;
			flex-direction: column;
			padding: 14px;
			gap: 12px;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
		}
	}
</style>
