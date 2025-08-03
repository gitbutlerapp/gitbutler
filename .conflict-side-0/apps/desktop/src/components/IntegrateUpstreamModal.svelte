<script lang="ts">
	import { writeClipboard } from '$lib/backend/clipboard';
	import { BASE_BRANCH_SERVICE } from '$lib/baseBranch/baseBranchService.svelte';
	import { DEFAULT_FORGE_FACTORY } from '$lib/forge/forgeFactory.svelte';
	import { type Stack } from '$lib/stacks/stack';
	import { TestId } from '$lib/testing/testIds';
	import {
		getBaseBranchResolution,
		type BaseBranchResolutionApproach,
		type Resolution,
		type StackStatus,
		stackFullyIntegrated,
		type BranchStatus,
		sortStatusInfoV3,
		getResolutionApproachV3,
		type StackStatusInfoV3,
		type StackStatusesWithBranchesV3
	} from '$lib/upstream/types';
	import { UPSTREAM_INTEGRATION_SERVICE } from '$lib/upstream/upstreamIntegrationService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import {
		Badge,
		Button,
		IntegrationSeriesRow,
		Modal,
		SimpleCommitRow,
		FileListItem,
		Select,
		SelectItem,
		ScrollableContainer,
		type BranchShouldBeDeletedMap
	} from '@gitbutler/ui';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { PullRequest } from '$lib/forge/interface/types';

	type OperationState = 'inert' | 'loading' | 'completed';
	type OperationType = 'rebase' | 'merge' | 'unapply' | 'delete';

	interface Props {
		projectId: string;
		onClose?: () => void;
	}

	const { projectId, onClose }: Props = $props();

	const upstreamIntegrationService = inject(UPSTREAM_INTEGRATION_SERVICE);
	const forge = inject(DEFAULT_FORGE_FACTORY);
	// const forgeListingService = $derived(forge.current.listService);
	const baseBranchService = inject(BASE_BRANCH_SERVICE);
	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchResponse.current.data);

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>('inert');
	const results = new SvelteMap<string, Resolution>();
	let statuses = $state<StackStatusInfoV3[]>([]);
	let baseResolutionApproach = $state<BaseBranchResolutionApproach | undefined>();
	let targetCommitOid = $state<string | undefined>(undefined);
	let branchStatuses = $state<StackStatusesWithBranchesV3 | undefined>();
	// const stackService = getContext(StackService);
	// let appliedBranches = $state<string[]>();
	// Any PRs belonging to applied branches that have been merged
	let filteredReviews = $state<PullRequest[]>([]);
	const reviewMap = $derived(new Map(filteredReviews?.map((r) => [r.sourceBranch, r])));

	const isDivergedResolved = $derived(base?.diverged && !baseResolutionApproach);
	const [integrateUpstream] = $derived(upstreamIntegrationService.integrateUpstream());

	$effect(() => {
		if (!modal?.imports.open) return;
		if (branchStatuses?.type !== 'updatesRequired') {
			statuses = [];
			return;
		}

		const statusesTmp = [...branchStatuses.subject];
		statusesTmp.sort(sortStatusInfoV3);

		// Side effect, refresh results
		results.clear();
		for (const status of statusesTmp) {
			const mergedAssociatedReviews = filteredReviews.filter(
				(r) => status.stack.heads.some((h) => h.name === r.sourceBranch) && r.mergedAt !== undefined
			);
			const forceIntegratedBranches = mergedAssociatedReviews.map((r) => r.sourceBranch);

			results.set(status.stack.id, {
				branchId: status.stack.id,
				approach: getResolutionApproachV3(status),
				deleteIntegratedBranches: true,
				forceIntegratedBranches
			});
		}

		statuses = statusesTmp;
	});

	// Re-fetch upstream statuses if the target commit oid changes
	$effect(() => {
		if (!modal?.imports.open) return;
		if (targetCommitOid) {
			upstreamIntegrationService.upstreamStatuses(projectId, targetCommitOid).then((statuses) => {
				branchStatuses = statuses;
			});
		}
	});

	// Resolve the target commit oid if the base branch diverged and the the resolution
	// approach is changed
	$effect(() => {
		if (!modal?.imports.open) return;
		if (base?.diverged && baseResolutionApproach) {
			upstreamIntegrationService
				.resolveUpstreamIntegrationMutation({
					projectId,
					resolutionApproach: { type: baseResolutionApproach }
				})
				.then((result) => {
					targetCommitOid = result;
				});
		} else {
			// If there is no divergence we should set this to undefined.
			targetCommitOid = undefined;
		}
	});

	// async function setFilteredBranches(appliedBranches: string[]) {
	// 	if (!forgeListingService) return;

	// 	try {
	// 		// Fetch the base branch and the forge info to ensure we have the
	// 		// latest data We only need to (and want to) do this if we are also
	// 		// looking at the reviews.
	// 		//
	// 		// This is to handle the case where the reviews might dictacte that
	// 		// we should remove a branch, but we don't have the have the merge
	// 		// commit yet. If we were to handle a branch as "integrated" without
	// 		// the merge commit, files might dissapear for a users working tree
	// 		// in a supprising way.
	// 		//
	// 		// We could query both of these simultaniously using Promise.all,
	// 		// but that is extra complexity that is not needed for now.
	// 		await baseBranchService.fetchFromRemotes(projectId);
	// 		const reviews = await forgeListingService.fetchByBranch(projectId, appliedBranches);

	// 		// Find the reviews that have a "mergedAt" timestamp
	// 		filteredReviews = reviews.filter((r) => !!r.mergedAt);
	// 	} catch (_e) {
	// 		// We don't really mind if this fails as additional bonus
	// 		// information.
	// 	}
	// }

	function handleBaseResolutionSelection(value: string) {
		baseResolutionApproach = value as BaseBranchResolutionApproach;
	}

	async function integrate() {
		integratingUpstream = 'loading';
		await tick();
		const baseResolution = getBaseBranchResolution(
			targetCommitOid,
			baseResolutionApproach || 'hardReset'
		);

		await integrateUpstream({
			projectId,
			resolutions: Array.from(results.values()),
			baseBranchResolution: baseResolution
		});
		await baseBranchService.refreshBaseBranch(projectId);
		integratingUpstream = 'completed';
		modal?.close();
	}

	// async function fetchAppliedBranches() {
	// 	const stacksResponse = await stackService.fetchStacks(projectId);
	// 	return stacksResponse.data?.flatMap((stack) => stack.heads.map((head) => head.name)) ?? [];
	// }

	export async function show() {
		integratingUpstream = 'inert';
		branchStatuses = undefined;
		filteredReviews = [];
		await tick();
		modal?.show();
		// appliedBranches = await fetchAppliedBranches();
		// await setFilteredBranches(untrack(() => appliedBranches) ?? []); // TODO: Some day this will be made good
		branchStatuses = await upstreamIntegrationService.upstreamStatuses(projectId, targetCommitOid);
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		}
	};

	function branchStatusToRowEntry(
		associatedeReview: PullRequest | undefined,
		branchStatus: BranchStatus
	): 'integrated' | 'conflicted' | 'clear' {
		if (associatedeReview?.mergedAt !== undefined) {
			return 'integrated';
		}

		if (branchStatus.type === 'integrated') {
			return 'integrated';
		}

		if (branchStatus.type === 'conflicted') {
			return 'conflicted';
		}

		return 'clear';
	}

	function integrationRowSeries(
		stackStatus: StackStatus
	): { name: string; status: 'integrated' | 'conflicted' | 'clear' }[] {
		const statuses = stackStatus.branchStatuses.map((series) => {
			const associatedeReview = reviewMap.get(series.name);
			return {
				name: series.name,
				status: branchStatusToRowEntry(associatedeReview, series.status)
			};
		});

		statuses.reverse();

		return statuses;
	}
	function getBranchShouldBeDeletedMap(
		stackId: string,
		stackStatus: StackStatus
	): BranchShouldBeDeletedMap {
		const branchShouldBeDeletedMap: BranchShouldBeDeletedMap = {};
		stackStatus.branchStatuses.forEach((branch) => {
			branchShouldBeDeletedMap[branch.name] = !!results.get(stackId)?.deleteIntegratedBranches;
		});
		return branchShouldBeDeletedMap;
	}

	function updateBranchShouldBeDeletedMap(stackId: string, shouldBeDeleted: boolean): void {
		const result = results.get(stackId);
		if (!result) return;
		results.set(stackId, { ...result, deleteIntegratedBranches: shouldBeDeleted });
	}

	function integrationOptions(
		stackStatus: StackStatus
	): { label: string; value: 'rebase' | 'unapply' | 'merge' }[] {
		if (stackStatus.branchStatuses.length > 1) {
			return [
				{ label: 'Rebase', value: 'rebase' },
				{ label: 'Stash', value: 'unapply' }
			];
		} else {
			return [
				{ label: 'Rebase', value: 'rebase' },
				{ label: 'Merge', value: 'merge' },
				{ label: 'Stash', value: 'unapply' }
			];
		}
	}
</script>

{#snippet stackStatus(stack: Stack, stackStatus: StackStatus)}
	{@const branchShouldBeDeletedMap = getBranchShouldBeDeletedMap(stack.id, stackStatus)}
	<IntegrationSeriesRow
		testId={TestId.IntegrateUpstreamSeriesRow}
		series={integrationRowSeries(stackStatus)}
		{branchShouldBeDeletedMap}
		updateBranchShouldBeDeletedMap={(_, shouldBeDeleted) =>
			updateBranchShouldBeDeletedMap(stack.id, shouldBeDeleted)}
	>
		{#if !stackFullyIntegrated(stackStatus) && results.get(stack.id)}
			<Select
				value={results.get(stack.id)!.approach.type}
				maxWidth={130}
				onselect={(value) => {
					const result = results.get(stack.id)!;
					results.set(stack.id, { ...result, approach: { type: value as OperationType } });
				}}
				options={integrationOptions(stackStatus)}
			>
				{#snippet itemSnippet({ item, highlighted })}
					<SelectItem selected={highlighted} {highlighted}>
						{item.label}
					</SelectItem>
				{/snippet}
			</Select>
		{/if}
	</IntegrationSeriesRow>
{/snippet}

<Modal
	testId={TestId.IntegrateUpstreamCommitsModal}
	bind:this={modal}
	{onClose}
	width={520}
	noPadding
	onSubmit={() => integrate()}
>
	<ScrollableContainer maxHeight="70vh">
		{#if base}
			<div class="section">
				<h3 class="text-14 text-semibold section-title">
					<span>Incoming changes</span><Badge>{base.upstreamCommits.length}</Badge>
				</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="{pxToRem(268)}rem">
						{#each base.upstreamCommits as commit}
							{@const commitUrl = forge.current.commitUrl(commit.id)}
							<SimpleCommitRow
								title={commit.descriptionTitle ?? ''}
								sha={commit.id}
								date={commit.createdAt}
								author={commit.author.name}
								url={commitUrl}
								onOpen={(url) => openExternalUrl(url)}
								onCopy={() => writeClipboard(commit.id)}
							/>
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}
		<!-- CONFLICTED FILES -->
		{#if branchStatuses?.type === 'updatesRequired' && branchStatuses?.worktreeConflicts.length > 0}
			<div class="section">
				<h3 class="text-14 text-semibold section-title">
					<span>Conflicting uncommitted files</span>

					<Badge>{branchStatuses?.worktreeConflicts.length}</Badge>
				</h3>
				<p class="text-12 clr-text-2">
					Updating the workspace will add conflict markers to the following files.
				</p>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="{pxToRem(268)}rem">
						{@const conflicts = branchStatuses?.worktreeConflicts}
						{#each conflicts as file}
							<FileListItem
								listMode="list"
								filePath={file}
								clickable={false}
								conflicted
								hideBorder={file === conflicts[conflicts.length - 1]}
							/>
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}
		<!-- DIVERGED -->
		{#if base?.diverged}
			<div class="target-divergence">
				<img class="target-icon" src="/images/domain-icons/trunk.svg" alt="" />

				<div class="target-divergence-about">
					<h3 class="text-14 text-semibold">Target branch divergence</h3>
					<p class="text-12 text-body target-divergence-description">
						<span class="text-bold">target/main</span> has diverged from the workspace.
						<br />
						Select an action to proceed with updating.
					</p>
				</div>

				<div class="target-divergence-action">
					<Select
						value={baseResolutionApproach}
						placeholder="Choose…"
						onselect={handleBaseResolutionSelection}
						options={[
							{ label: 'Rebase', value: 'rebase' },
							{ label: 'Merge', value: 'merge' },
							{ label: 'Hard reset', value: 'hardReset' }
						]}
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem selected={highlighted} {highlighted}>
								{item.label}
							</SelectItem>
						{/snippet}
					</Select>
				</div>
			</div>
		{/if}
		<!-- STACKS AND BRANCHES TO UPDATE -->
		{#if statuses.length > 0}
			<div class="section" class:section-disabled={isDivergedResolved}>
				<h3 class="text-14 text-semibold">To be updated:</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="{pxToRem(240)}rem">
						{#each statuses as { stack, status }}
							{@render stackStatus(stack, status)}
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}
	</ScrollableContainer>

	{#snippet controls()}
		<div class="controls">
			<Button onclick={() => modal?.close()} kind="outline">Cancel</Button>
			<Button
				testId={TestId.IntegrateUpstreamActionButton}
				wide
				style="pop"
				disabled={isDivergedResolved || !branchStatuses}
				loading={integratingUpstream === 'loading' || !branchStatuses}
				onclick={async () => {
					await integrate();
				}}
			>
				Update workspace
			</Button>
		</div>
	{/snippet}
</Modal>

<style>
	/* INCOMING CHANGES */
	.section {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 14px;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}

		.scroll-wrap {
			overflow: hidden;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
		}
	}

	.section-title {
		display: flex;
		align-items: center;
		gap: 4px;
	}

	/* DIVERGANCE */
	.target-divergence {
		display: flex;
		padding: 16px;
		gap: 14px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-theme-warn-bg);
	}

	.target-icon {
		width: 16px;
		height: 16px;
		border-radius: var(--radius-s);
	}

	.target-divergence-about {
		display: flex;
		flex-direction: column;
		width: 100%;
		gap: 8px;
	}

	.target-divergence-description {
		color: var(--clr-text-2);
	}

	.target-divergence-action {
		display: flex;
		flex-direction: column;
		max-width: 230px;
	}

	/* CONTROLS */
	.controls {
		display: flex;
		width: 100%;
		gap: 6px;
	}

	/* MODIFIERS */
	.section-disabled {
		opacity: 0.5;
		pointer-events: none;
	}
</style>
