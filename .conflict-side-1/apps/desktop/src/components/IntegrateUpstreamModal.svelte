<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { Project } from '$lib/project/project';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import {
		getBaseBranchResolution,
		getResolutionApproach,
		sortStatusInfo,
		type BaseBranchResolutionApproach,
		type StackStatusesWithBranches,
		type StackStatusInfo,
		type Resolution,
		type StackStatus,
		stackFullyIntegrated,
		type BranchStatus
	} from '$lib/upstream/types';
	import { UpstreamIntegrationService } from '$lib/upstream/upstreamIntegrationService';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import IntegrationSeriesRow, {
		type BranchShouldBeDeletedMap
	} from '@gitbutler/ui/IntegrationSeriesRow.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SimpleCommitRow from '@gitbutler/ui/SimpleCommitRow.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { PullRequest } from '$lib/forge/interface/types';

	type OperationState = 'inert' | 'loading' | 'completed';
	type OperationType = 'rebase' | 'merge' | 'unapply' | 'delete';

	interface Props {
		onClose?: () => void;
	}

	const { onClose }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const forgeListingService = $derived(forge.current.listService);
	const project = getContext(Project);
	const projectId = $derived(project.id);
	const upstreamIntegrationService = getContext(UpstreamIntegrationService);
	let branchStatuses = $state<StackStatusesWithBranches | undefined>();
	const baseBranchService = getContext(BaseBranchService);
	const baseResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseResponse.current.data);

	const stackService = getContext(StackService);
	let appliedBranches = $state<string[]>();
	let filteredReviews = $state<PullRequest[] | undefined>(undefined);
	const reviewMap = $derived(new Map(filteredReviews?.map((r) => [r.sourceBranch, r])));

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>('inert');
	const results = new SvelteMap<string, Resolution>();
	let statuses = $state<StackStatusInfo[]>([]);
	let baseResolutionApproach = $state<BaseBranchResolutionApproach | undefined>();
	let targetCommitOid = $state<string | undefined>(undefined);

	const isDivergedResolved = $derived(base?.diverged && !baseResolutionApproach);

	$effect(() => {
		if (!modal?.imports.open) return;
		if (branchStatuses?.type !== 'updatesRequired' || filteredReviews === undefined) {
			statuses = [];
			return;
		}

		const statusesTmp = [...branchStatuses.subject];
		statusesTmp.sort(sortStatusInfo);

		// Side effect, refresh results
		results.clear();
		for (const status of statusesTmp) {
			const mergedAssociatedReviews = filteredReviews.filter(
				(r) =>
					status.status.branchStatuses.some((series) => series.name === r.sourceBranch) &&
					r.mergedAt !== undefined
			);
			const forceIntegratedBranches = mergedAssociatedReviews.map((r) => r.sourceBranch);

			const defaultApproach = getResolutionApproach(status);
			results.set(status.stack.id, {
				branchId: status.stack.id,
				approach: defaultApproach,
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
			upstreamIntegrationService.upstreamStatuses(targetCommitOid).then((statuses) => {
				branchStatuses = statuses;
			});
		}
	});

	// Resolve the target commit oid if the base branch diverged and the the resolution
	// approach is changed
	$effect(() => {
		if (!modal?.imports.open) return;
		if (base?.diverged && baseResolutionApproach) {
			upstreamIntegrationService.resolveUpstreamIntegration(baseResolutionApproach).then((Oid) => {
				targetCommitOid = Oid;
			});
		}
	});

	// Fetch the reviews for the applied branches
	$effect(() => {
		if (!modal?.imports.open) return;
		if (
			appliedBranches !== undefined &&
			filteredReviews === undefined &&
			forgeListingService !== undefined
		) {
			if (appliedBranches.length === 0) {
				filteredReviews = [];
				return;
			}

			forgeListingService.fetchByBranch(projectId, appliedBranches).then((reviews) => {
				filteredReviews = reviews.filter((r) => r.sourceBranch !== undefined);
			});
		}
	});

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

		await upstreamIntegrationService.integrateUpstream(
			Array.from(results.values()),
			baseResolution
		);
		await baseBranchService.refreshBaseBranch(projectId);
		integratingUpstream = 'completed';

		modal?.close();
	}
	async function fetchAppliedBranches() {
		return await stackService
			.fetchStacks(projectId)
			.then(
				(stacksResponse) =>
					stacksResponse.data?.flatMap((stack) => stack.heads.map((head) => head.name)) ?? []
			);
	}

	export async function show() {
		integratingUpstream = 'inert';
		branchStatuses = undefined;
		filteredReviews = undefined;
		await tick();
		modal?.show();
		// Fetch the base branch and the forge info to ensure we have the latest data
		await baseBranchService.fetchFromRemotes(projectId);
		appliedBranches = await fetchAppliedBranches();
		branchStatuses = await upstreamIntegrationService.upstreamStatuses();
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

{#snippet stackStatus(stack: BranchStack, stackStatus: StackStatus)}
	{@const series = integrationRowSeries(stackStatus)}
	{@const branchShouldBeDeletedMap = getBranchShouldBeDeletedMap(stack.id, stackStatus)}
	<IntegrationSeriesRow
		{series}
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

<Modal bind:this={modal} {onClose} width={520} noPadding onSubmit={integrate}>
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

		{#if statuses.length > 0}
			<div class="section" class:section-disabled={isDivergedResolved}>
				<h3 class="text-14 text-semibold">To be updated:</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight="{pxToRem(240)}rem">
						<div>
							{#each statuses as { stack, status }}
								{@render stackStatus(stack, status)}
							{/each}
						</div>
					</ScrollableContainer>
				</div>
			</div>
		{/if}
	</ScrollableContainer>

	{#snippet controls()}
		<div class="controls">
			<Button onclick={() => modal?.close()} kind="outline">Cancel</Button>
			<Button
				wide
				type="submit"
				style="pop"
				disabled={isDivergedResolved || !branchStatuses}
				loading={integratingUpstream === 'loading' ||
					!branchStatuses ||
					filteredReviews === undefined}>Update workspace</Button
			>
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

		& .scroll-wrap {
			overflow: hidden;
			border: 1px solid var(--clr-border-2);
			border-radius: var(--radius-m);
		}

		&:nth-last-child(2) {
			border-bottom: none;
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
