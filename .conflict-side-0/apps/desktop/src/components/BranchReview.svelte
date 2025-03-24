<script lang="ts">
	import BranchReviewButRequest from '$components/BranchReviewButRequest.svelte';
	import { syncBrToPr } from '$lib/forge/brToPrSync.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { syncPrToBr } from '$lib/forge/prToBrSync.svelte';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';
	import type { Snippet } from 'svelte';

	// TODO: This and the SeriesHeader should have a wholistic refactor to
	// reduce the complexity of the forge related functionality.

	type Props = {
		pullRequestCard?: Snippet<[DetailedPullRequest]>;
		branchStatus?: Snippet;
		openForgePullRequest: () => void;
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const {
		pullRequestCard,
		branchStatus,
		openForgePullRequest,
		projectId,
		stackId,
		branchName
	}: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const stackPublishingService = getContext(StackPublishingService);
	const stackService = getContext(StackService);

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

	const showCreateButton = $derived((prService && !pr) || ($canPublish && !reviewId));

	const disabled = $derived(branchEmpty || branchConflicted);
	const tooltip = $derived(
		branchConflicted ? 'Please resolve the conflicts before creating a PR' : undefined
	);

	syncPrToBr(
		reactive(() => prNumber || undefined),
		reactive(() => reviewId)
	);
	syncBrToPr(
		reactive(() => prNumber),
		reactive(() => reviewId)
	);
</script>

<div class="branch-action">
	{#if pr || reviewId}
		<div class="status-cards">
			{#if pr && pullRequestCard}
				<div>
					{@render pullRequestCard(pr)}
				</div>
			{/if}
			{#if reviewId}
				<div>
					<BranchReviewButRequest {reviewId} />
				</div>
			{/if}
		</div>
	{/if}

	{#if branchStatus}
		{@render branchStatus()}
	{/if}

	{#if showCreateButton}
		<Button onclick={openForgePullRequest} kind="outline" {disabled} {tooltip}>
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

	.branch-action:not(:has(> *)) {
		padding: 0;
	}

	.status-cards {
		display: flex;
		flex-direction: column;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);

		> div {
			padding: 14px;

			&:not(:last-child) {
				border-bottom: 1px solid var(--clr-border-2);
			}
		}
	}
</style>
