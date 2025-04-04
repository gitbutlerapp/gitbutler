<script lang="ts">
	import BranchReviewButRequest from '$components/BranchReviewButRequest.svelte';
	import PullRequestCard from '$components/PullRequestCard.svelte';
	import ReviewCreation from '$components/ReviewCreation.svelte';
	import { SettingsService } from '$lib/config/appSettingsV2';
	import { syncBrToPr } from '$lib/forge/brToPrSync.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { syncPrToBr } from '$lib/forge/prToBrSync.svelte';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';
	import type { Snippet } from 'svelte';

	// TODO: This and the SeriesHeader should have a wholistic refactor to
	// reduce the complexity of the forge related functionality.

	type Props = {
		pullRequestCard?: Snippet<[DetailedPullRequest]>;
		branchStatus?: Snippet;
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { branchStatus, projectId, stackId, branchName }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const stackPublishingService = getContext(StackPublishingService);
	const stackService = getContext(StackService);
	const uiState = getContext(UiState);
	const settingsService = getContext(SettingsService);
	const settingsStore = settingsService.appSettings;

	const branch = $derived(stackService.branchByName(projectId, stackId, branchName));
	const commits = $derived(stackService.commits(projectId, stackId, branchName));

	const prNumber = $derived(branch.current.data?.prNumber ?? undefined);
	const reviewId = $derived(branch.current.data?.reviewId ?? undefined);
	const branchEmpty = $derived((commits.current.data?.length ?? 0) === 0);
	const branchConflicted = $derived(
		commits.current.data?.some((commit) => commit.hasConflicts) || false
	);

	const prService = $derived(forge.current.prService);
	const prResult = $derived(prNumber ? prService?.get(prNumber) : undefined);
	const pr = $derived(prResult?.current.data);

	const canPublish = stackPublishingService.canPublish;

	const canPublishBR = $derived(
		!!($canPublish && branch.current.data?.name && !branch.current.data?.reviewId)
	);
	const canPublishPR = $derived(!!(forge.current.authenticated && !pr));
	const showCreateButton = $derived(canPublishBR || canPublishPR);

	const disabled = $derived(branchEmpty || branchConflicted);
	const tooltip = $derived(
		branchConflicted ? 'Please resolve the conflicts before creating a PR' : undefined
	);

	let modal = $state<Modal>();
	let confirmCreatePrModal = $state<ReturnType<typeof Modal>>();
	let reviewCreation = $state<ReviewCreation>();

	syncPrToBr(
		reactive(() => prNumber),
		reactive(() => reviewId)
	);
	syncBrToPr(
		reactive(() => prNumber),
		reactive(() => reviewId)
	);
</script>

<Modal
	width="small"
	type="warning"
	title="Create Pull Request"
	bind:this={confirmCreatePrModal}
	onSubmit={() => {
		modal?.show();
	}}
>
	{#snippet children()}
		<p class="text-13 text-body helper-text">
			It's strongly recommended to create pull requests starting with the branch at the base of the
			stack.
			<br />
			Do you still want to create this pull request?
		</p>
	{/snippet}
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
		<Button kind="outline" onclick={close}>Close</Button>
		<AsyncButton
			style="pop"
			loading={reviewCreation?.imports.isLoading}
			action={async () => await reviewCreation?.createReview()}
			disabled={!reviewCreation?.createButtonEnabled().current}
		>
			Create Review
		</AsyncButton>
	{/snippet}
</Modal>

<div class="branch-action">
	{#if pr || (reviewId && $canPublish)}
		<div class="status-cards">
			{#if prNumber}
				<PullRequestCard {projectId} {stackId} {branchName} poll />
			{/if}
			{#if reviewId && $canPublish}
				<BranchReviewButRequest {reviewId} />
			{/if}
		</div>
	{/if}

	{#if branchStatus}
		{@render branchStatus()}
	{/if}

	{#if showCreateButton}
		<Button
			onclick={() => {
				if ($settingsStore?.featureFlags.v3) {
					uiState.project(projectId).drawerPage.current = 'review';
				} else {
					modal?.show();
				}
			}}
			kind="outline"
			{disabled}
			{tooltip}
		>
			Submit for Review
		</Button>
	{/if}
</div>

<style lang="postcss">
	.branch-action {
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}

	/*
		The :empty selector does not work in svelte because undeterminate reasons.
		As such we have this beauty.

		All we want to do is to have this thing to not add extra whitespace if
		there is nothing interesting going on inside of the component.

		We don't want to use display: none as that breaks things in other strange ways
	*/

	/* .branch-action:not(:has(> *)) {
		padding: 0;
	} */

	.status-cards {
		display: flex;
		flex-direction: column;
		gap: 8px;

		& :global(.review-card) {
			position: relative;
			display: flex;
			flex-direction: column;
			gap: 12px;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
			padding: 14px;
		}
	}
</style>
