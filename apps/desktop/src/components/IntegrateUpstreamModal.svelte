<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { writeClipboard } from '$lib/backend/clipboard';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { BranchStack } from '$lib/branches/branch';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { Project } from '$lib/project/project';
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
	import IntegrationSeriesRow from '@gitbutler/ui/IntegrationSeriesRow.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SimpleCommitRow from '@gitbutler/ui/SimpleCommitRow.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';

	type OperationState = 'inert' | 'loading' | 'completed';
	type OperationType = 'rebase' | 'merge' | 'unapply' | 'delete';

	interface Props {
		onClose?: () => void;
	}

	const { onClose }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const project = getContext(Project);
	const projectId = $derived(project.id);
	const upstreamIntegrationService = getContext(UpstreamIntegrationService);
	let branchStatuses = $state<StackStatusesWithBranches | undefined>();
	const baseBranchService = getContext(BaseBranchService);
	const baseResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseResponse.current.data);

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>('inert');
	let results = $state(new SvelteMap<string, Resolution>());
	let statuses = $state<StackStatusInfo[]>([]);
	let baseResolutionApproach = $state<BaseBranchResolutionApproach | undefined>();
	let targetCommitOid = $state<string | undefined>(undefined);

	const isDivergedResolved = $derived(base?.diverged && !baseResolutionApproach);

	$effect(() => {
		if (branchStatuses?.type !== 'updatesRequired') {
			statuses = [];
			return;
		}

		const statusesTmp = [...branchStatuses.subject];
		statusesTmp.sort(sortStatusInfo);

		// Side effect, refresh results
		results = new SvelteMap(
			statusesTmp.map((status) => {
				const defaultApproach = getResolutionApproach(status);

				return [
					status.stack.id,
					{
						branchId: status.stack.id,
						approach: defaultApproach,
						deleteIntegratedBranches: false // TODO: Take input from the UI
					}
				];
			})
		);

		statuses = statusesTmp;
	});

	// Re-fetch upstream statuses if the target commit oid changes
	$effect(() => {
		if (targetCommitOid) {
			upstreamIntegrationService.upstreamStatuses(targetCommitOid).then((statuses) => {
				branchStatuses = statuses;
			});
		}
	});

	// Resolve the target commit oid if the base branch diverged and the the resolution
	// approach is changed
	$effect(() => {
		if (base?.diverged && baseResolutionApproach) {
			upstreamIntegrationService.resolveUpstreamIntegration(baseResolutionApproach).then((Oid) => {
				targetCommitOid = Oid;
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

	export async function show() {
		integratingUpstream = 'inert';
		branchStatuses = undefined;
		modal?.show();
		branchStatuses = await upstreamIntegrationService.upstreamStatuses();
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
	<IntegrationSeriesRow series={integrationRowSeries(stackStatus)}>
		{#snippet select()}
			{#if !stackFullyIntegrated(stackStatus) && results.get(stack.id)}
				<Select
					value={results.get(stack.id)!.approach.type}
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
		{/snippet}
	</IntegrationSeriesRow>
{/snippet}

<Modal bind:this={modal} {onClose} width={520} noPadding onSubmit={integrate}>
	<ScrollableContainer maxHeight={'70vh'}>
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
					<ScrollableContainer maxHeight={pxToRem(240)}>
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

		& .scroll-wrap {
			border-radius: var(--radius-m);
			border: 1px solid var(--clr-border-2);
			overflow: hidden;
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
