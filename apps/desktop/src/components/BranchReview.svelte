<script lang="ts">
	import BranchReviewButRequest from '$components/BranchReviewButRequest.svelte';
	import { BranchStack, type PatchSeries } from '$lib/branches/branch';
	import { BranchController } from '$lib/branches/branchController';
	import { cloudReviewFunctionality } from '$lib/config/uiFeatureFlags';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { getPr } from '$lib/pr/getPr.svelte';
	import { getContextStore, inject } from '@gitbutler/shared/context';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
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

	const stack = getContextStore(BranchStack);
	const [stackPublishingService, branchController] = inject(
		StackPublishingService,
		BranchController
	);

	async function publishReview() {
		await branchController.pushBranch($stack.id, true);
		await stackPublishingService.upsertStack($stack.id, branch.name);
	}

	const prService = getForgePrService();
	const pr = getPr(reactive(() => branch));

	const enum CreationAction {
		CreateBR,
		CreatePR
	}

	const creationActionsDisplay = {
		[CreationAction.CreateBR]: 'Create butler review',
		[CreationAction.CreatePR]: 'Create pull request'
	};

	let selectedAction = $state<CreationAction>();

	const actions = $derived.by(() => {
		const out: CreationAction[] = [];
		if ($prService && !pr.current) {
			out.push(CreationAction.CreatePR);
		}
		if (stackPublishingService.canPublish && !branch.reviewId && $cloudReviewFunctionality) {
			out.push(CreationAction.CreateBR);
		}
		return out;
	});

	$effect(() => {
		selectedAction = actions.at(0);
	});

	let loading = $state(false);

	async function create(action?: CreationAction) {
		if (!isDefined(action)) return;
		loading = true;

		try {
			switch (action) {
				case CreationAction.CreatePR:
					await openForgePullRequest();
					break;
				case CreationAction.CreateBR:
					await publishReview();
					break;
			}
		} finally {
			loading = false;
		}
	}

	const disabled = $derived(branch.patches.length === 0 || branch.conflicted);
	const tooltip = $derived(
		branch.conflicted ? 'Please resolve the conflicts before creating a PR' : undefined
	);

	let dropDownButton = $state<DropDownButton>();
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

		{#if actions.length > 0 && isDefined(selectedAction)}
			{#if actions.length > 1}
				<DropDownButton
					style="neutral"
					kind="outline"
					onclick={() => create(selectedAction)}
					{loading}
					bind:this={dropDownButton}
				>
					{creationActionsDisplay[selectedAction]}
					{#snippet contextMenuSlot()}
						<ContextMenuSection>
							{#each actions as action}
								<ContextMenuItem
									label={creationActionsDisplay[action]}
									onclick={() => {
										selectedAction = action;
										dropDownButton?.close();
									}}
								/>
							{/each}
						</ContextMenuSection>
					{/snippet}
				</DropDownButton>
			{:else}
				<Button
					onclick={() => create(selectedAction)}
					{disabled}
					{tooltip}
					{loading}
					style="neutral"
					kind="outline"
				>
					{creationActionsDisplay[selectedAction]}
				</Button>
			{/if}
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
