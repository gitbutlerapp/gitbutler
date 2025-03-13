<script lang="ts">
	import BranchReviewButRequest from '$components/BranchReviewButRequest.svelte';
	import { type PatchSeries } from '$lib/branches/branch';
	import { syncBrToPr } from '$lib/forge/brToPrSync.svelte';
	import { getPr } from '$lib/forge/getPr.svelte';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { syncPrToBr } from '$lib/forge/prToBrSync.svelte';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { inject } from '@gitbutler/shared/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';
	import type { Snippet } from 'svelte';

	// TODO: This and the SeriesHeader should have a wholistic refactor to
	// reduce the complexity of the forge related functionality.

	type Props = {
		pullRequestCard: Snippet<[DetailedPullRequest]>;
		branchStatus: Snippet;
		branchLine: Snippet;
		openForgePullRequest: () => void;
		branch: PatchSeries;
	};

	const { pullRequestCard, branchStatus, branchLine, branch, openForgePullRequest }: Props =
		$props();

	const [stackPublishingService] = inject(StackPublishingService);

	const prService = getForgePrService();
	const pr = getPr(reactive(() => branch));

	const canPublish = stackPublishingService.canPublish;

	const showCreateButton = $derived(
		($prService && !pr.current) || ($canPublish && !branch.reviewId)
	);

	const disabled = $derived(branch.patches.length === 0 || branch.conflicted);
	const tooltip = $derived(
		branch.conflicted ? 'Please resolve the conflicts before creating a PR' : undefined
	);

	syncPrToBr(reactive(() => branch));
	syncBrToPr(reactive(() => branch));
</script>

<div class="branch-action">
	{@render branchLine()}
	<div class="branch-action__body">
		{#if pr.current || branch.reviewId}
			<div class="status-cards">
				{#if pr.current}
					<div>
						{@render pullRequestCard(pr.current)}
					</div>
				{/if}
				{#if branch.reviewId}
					<div>
						<BranchReviewButRequest reviewId={branch.reviewId} />
					</div>
				{/if}
			</div>
		{/if}

		{@render branchStatus()}

		{#if showCreateButton}
			<Button onclick={openForgePullRequest} kind="outline" {disabled} {tooltip}
				>Submit for Review</Button
			>
		{/if}
	</div>
</div>

<style lang="postcss">
	.branch-action {
		width: 100%;
		display: flex;
		justify-content: flex-start;
		align-items: stretch;

		.branch-action__body {
			width: 100%;
			padding: 0 14px 14px 0;
			display: flex;
			flex-direction: column;
			gap: 14px;
		}
	}

	/*
		The :empty selector does not work in svelte because undeterminate reasons.
		As such we have this beauty.

		All we want to do is to have this thing to not add extra whitespace if
		there is nothing interesting going on inside of the component.

		We don't want to use display: none as that breaks things in other strange ways
	*/

	.branch-action:not(:has(> .branch-action__body > *)) {
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
