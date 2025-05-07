<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
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
		type StackStatusInfoV3
	} from '$lib/upstream/types';
	import { UpstreamIntegrationService } from '$lib/upstream/upstreamIntegrationService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import IntegrationSeriesRow, {
		type BranchShouldBeDeletedMap
	} from '@gitbutler/ui/IntegrationSeriesRow.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SimpleCommitRow from '@gitbutler/ui/SimpleCommitRow.svelte';
	import FileListItemV3 from '@gitbutler/ui/file/FileListItemV3.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';

	type OperationState = 'inert' | 'loading' | 'completed';
	type OperationType = 'rebase' | 'merge' | 'unapply' | 'delete';

	interface Props {
		projectId: string;
		onClose?: () => void;
	}

	const { projectId, onClose }: Props = $props();

	const upstreamIntegrationService = getContext(UpstreamIntegrationService);
	const forge = getContext(DefaultForgeFactory);
	const baseBranchService = getContext(BaseBranchService);
	const baseBranchResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseBranchResponse.current.data);
	const [resolveUpstreamIntegration] = upstreamIntegrationService.resolveUpstreamIntegration();

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>('inert');
	const results = new SvelteMap<string, Resolution>();
	let statuses = $state<StackStatusInfoV3[]>([]);
	let baseResolutionApproach = $state<BaseBranchResolutionApproach | undefined>();
	let targetCommitOid = $state<string | undefined>(undefined);

	const isDivergedResolved = $derived(base?.diverged && !baseResolutionApproach);
	const [integrateUpstream] = $derived(upstreamIntegrationService.integrateUpstream(projectId));

	// Will re-fetch upstream statuses if the target commit oid changes
	const branchStatuses = $derived(
		upstreamIntegrationService.upstreamStatuses(projectId, targetCommitOid)
	);

	$effect(() => {
		if (branchStatuses.current?.type !== 'updatesRequired') {
			statuses = [];
			return;
		}

		const statusesTmp = [...branchStatuses.current.subject];
		statusesTmp.sort(sortStatusInfoV3);

		// Side effect, refresh results
		results.clear();
		for (const status of statusesTmp) {
			results.set(status.stack.id, {
				branchId: status.stack.id,
				approach: getResolutionApproachV3(status),
				deleteIntegratedBranches: true
			});
		}

		statuses = statusesTmp;
	});

	// Resolve the target commit oid if the base branch diverged and the the resolution
	// approach is changed
	$effect(() => {
		if (base?.diverged && baseResolutionApproach) {
			resolveUpstreamIntegration({
				projectId,
				resolutionApproach: { type: baseResolutionApproach }
			}).then((result) => {
				if (result) {
					targetCommitOid = result;
				}
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

		try {
			await integrateUpstream({
				projectId,
				resolutions: Array.from(results.values()),
				baseBranchResolution: baseResolution
			});
		} finally {
			await baseBranchService.refreshBaseBranch(projectId);
			integratingUpstream = 'completed';
			modal?.close();
		}
	}

	export async function show() {
		integratingUpstream = 'inert';
		modal?.show();
		targetCommitOid = undefined;
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		}
	};

	function branchStatusToRowEntry(
		branchStatus: BranchStatus
	): 'integrated' | 'conflicted' | 'clear' {
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
		const statuses = stackStatus.branchStatuses.map((series) => ({
			name: series.name,
			status: branchStatusToRowEntry(series.status)
		}));

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
	onSubmit={integrate}
>
	<ScrollableContainer maxHeight="70vh">
		{#if base}
			<div class="section">
				<h3 class="text-14 text-semibold section-title">
					<span>Incoming changes</span><Badge>{base.upstreamCommits.length}</Badge>
				</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight={pxToRem(268)}>
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
		{#if branchStatuses.current?.type === 'updatesRequired' && branchStatuses.current?.worktreeConflicts.length > 0}
			<div class="section">
				<h3 class="text-14 text-semibold section-title">
					<span>Conflicting uncommitted files</span>

					<Badge>{branchStatuses.current?.worktreeConflicts.length}</Badge>
				</h3>
				<p class="text-12 text-clr2">
					Updating the workspace will add conflict markers to the following files.
				</p>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight={pxToRem(268)}>
						{@const conflicts = branchStatuses.current?.worktreeConflicts}
						{#each conflicts as file}
							<FileListItemV3
								listMode="list"
								filePath={file}
								clickable={false}
								conflicted
								isLast={file === conflicts[conflicts.length - 1]}
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
					<ScrollableContainer maxHeight={pxToRem(240)}>
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
				type="submit"
				style="pop"
				disabled={isDivergedResolved || !branchStatuses}
				loading={integratingUpstream === 'loading' || !branchStatuses}>Update workspace</Button
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

		&:last-child {
			border-bottom: none;
		}

		.scroll-wrap {
			border-radius: var(--radius-m);
			border: 1px solid var(--clr-border-2);
			overflow: hidden;
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
		width: 100%;
		flex-direction: column;
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
