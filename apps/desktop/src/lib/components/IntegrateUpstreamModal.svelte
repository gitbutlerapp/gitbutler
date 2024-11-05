<script lang="ts">
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { getForge } from '$lib/forge/interface/forge';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { openExternalUrl } from '$lib/utils/url';
	import {
		getBaseBrancheResolution,
		getResolutionApproach,
		sortStatusInfo,
		UpstreamIntegrationService,
		type BaseBranchResolutionApproach,
		type BranchStatusesWithBranches,
		type BranchStatusInfo,
		type Resolution
	} from '$lib/vbranches/upstreamIntegrationService';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import IntegrationSeriesRow from '@gitbutler/ui/IntegrationSeriesRow.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import SimpleCommitRow from '@gitbutler/ui/SimpleCommitRow.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { Readable } from 'svelte/store';

	type OperationState = 'inert' | 'loading' | 'completed';

	interface Props {
		onClose?: () => void;
	}

	const { onClose }: Props = $props();

	const forge = getForge();
	const upstreamIntegrationService = getContext(UpstreamIntegrationService);
	let branchStatuses = $state<Readable<BranchStatusesWithBranches | undefined>>();
	const baseBranchService = getContext(BaseBranchService);
	const base = baseBranchService.base;

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>('inert');
	let results = $state(new SvelteMap<string, Resolution>());
	let statuses = $state<BranchStatusInfo[]>([]);
	let baseResolutionApproach = $state<BaseBranchResolutionApproach | undefined>();
	let targetCommitOid = $state<string | undefined>(undefined);

	let isDivergedResolved = $derived($base?.diverged && !baseResolutionApproach);

	$effect(() => {
		if ($branchStatuses?.type !== 'updatesRequired') {
			statuses = [];
			return;
		}

		const statusesTmp = [...$branchStatuses.subject];
		statusesTmp.sort(sortStatusInfo);

		// Side effect, refresh results
		results = new SvelteMap(
			statusesTmp.map((status) => {
				const defaultApproach = getResolutionApproach(status);

				return [
					status.branch.id,
					{
						branchId: status.branch.id,
						branchTree: status.branch.tree,
						approach: defaultApproach
					}
				];
			})
		);

		statuses = statusesTmp;
	});

	// Re-fetch upstream statuses if the target commit oid changes
	$effect(() => {
		if (targetCommitOid) {
			branchStatuses = upstreamIntegrationService.upstreamStatuses(targetCommitOid);
		}
	});

	// Resolve the target commit oid if the base branch diverged and the the resolution
	// approach is changed
	$effect(() => {
		if ($base?.diverged && baseResolutionApproach) {
			upstreamIntegrationService.resolveUpstreamIntegration(baseResolutionApproach).then((Oid) => {
				targetCommitOid = Oid;
			});
		}
	});

	function handleBaseResolutionSelection(resolution: BaseBranchResolutionApproach) {
		baseResolutionApproach = resolution;
	}

	async function integrate() {
		integratingUpstream = 'loading';
		await tick();
		const baseResolution = getBaseBrancheResolution(
			targetCommitOid,
			baseResolutionApproach || 'hardReset'
		);
		await upstreamIntegrationService.integrateUpstream(
			Array.from(results.values()),
			baseResolution
		);
		await baseBranchService.refresh();
		integratingUpstream = 'completed';

		modal?.close();
	}

	export async function show() {
		integratingUpstream = 'inert';
		branchStatuses = upstreamIntegrationService.upstreamStatuses();
		await tick();
		modal?.show();
	}

	export const imports = {
		get open() {
			return modal?.imports.open;
		}
	};
</script>

<Modal bind:this={modal} {onClose} width={520} noPadding onSubmit={integrate}>
	<ScrollableContainer maxHeight={'70vh'}>
		{#if $base}
			<div class="section">
				<h3 class="text-14 text-semibold section-title">
					<span>Incoming changes</span><Badge label={$base.upstreamCommits.length} />
				</h3>
				<div class="scroll-wrap">
					<ScrollableContainer maxHeight={pxToRem(268)}>
						{#each $base.upstreamCommits as commit}
							<SimpleCommitRow
								title={commit.descriptionTitle}
								sha={commit.id}
								date={commit.createdAt}
								author={commit.author.name}
								onUrlOpen={() => {
									if ($forge) {
										openExternalUrl($forge.commitUrl(commit.id));
									}
								}}
								onCopy={() => {
									copyToClipboard(commit.id);
								}}
							/>
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}

		{#if $base?.diverged}
			<div class="target-divergence">
				<img class="target-icon" src="/images/domain-icons/trunk.svg" alt="" />

				<div class="target-divergence-about">
					<h3 class="text-14 text-semibold">Target branch divergence</h3>
					<p class="text-12 text-body target-divergence-description">
						Branch target/main has diverged from the workspace.
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
						{#each statuses as { branch, status }}
							<IntegrationSeriesRow
								type={status.type === 'fullyIntegrated'
									? 'integrated'
									: status.type === 'conflicted'
										? 'conflicted'
										: 'clear'}
								title={branch.name}
							>
								{#snippet select()}
									{#if status.type !== 'fullyIntegrated' && results.get(branch.id)}
										<Select
											value={results.get(branch.id)!.approach.type}
											onselect={(value) => {
												const result = results.get(branch.id)!;

												results.set(branch.id, { ...result, approach: { type: value } });
											}}
											options={[
												{ label: 'Rebase', value: 'rebase' },
												{ label: 'Merge', value: 'merge' },
												{ label: 'Stash', value: 'unapply' }
											]}
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
						{/each}
					</ScrollableContainer>
				</div>
			</div>
		{/if}
	</ScrollableContainer>

	{#snippet controls()}
		<div class="controls">
			<Button onclick={() => modal?.close()} style="ghost" outline>Cancel</Button>
			<Button
				wide
				type="submit"
				style="pop"
				kind="solid"
				disabled={isDivergedResolved}
				loading={integratingUpstream === 'loading'}>Update workspace</Button
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
