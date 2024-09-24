<script lang="ts">
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import CommitCard from '$lib/commit/CommitCard.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import { getContext } from '$lib/utils/context';
	import {
		getResolutionApproach,
		sortStatusInfo,
		UpstreamIntegrationService,
		type BranchStatusesWithBranches,
		type BranchStatusInfo,
		type Resolution
	} from '$lib/vbranches/upstreamIntegrationService';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { tick } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { Readable } from 'svelte/store';

	type OperationState = 'inert' | 'loading' | 'completed';

	interface Props {
		onClose?: () => void;
	}

	const { onClose }: Props = $props();

	const gitHost = getGitHost();
	const upstreamIntegrationService = getContext(UpstreamIntegrationService);
	let branchStatuses = $state<Readable<BranchStatusesWithBranches | undefined>>();
	const baseBranchService = getContext(BaseBranchService);
	const base = baseBranchService.base;

	let modal = $state<Modal>();
	let integratingUpstream = $state<OperationState>('inert');
	let results = $state(new SvelteMap<string, Resolution>());
	let statuses = $state<BranchStatusInfo[]>([]);
	let expanded = $state<boolean>(false);

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

	async function integrate() {
		integratingUpstream = 'loading';
		await tick();
		await upstreamIntegrationService.integrateUpstream(Array.from(results.values()));
		await baseBranchService.refresh();
		integratingUpstream = 'completed';

		modal?.close();
	}

	export async function show() {
		integratingUpstream = 'inert';
		expanded = false;
		branchStatuses = upstreamIntegrationService.upstreamStatuses();
		await tick();
		modal?.show();
	}
</script>

<Modal bind:this={modal} title="Integrate upstream changes" {onClose} width="small" noPadding>
	<ScrollableContainer maxHeight="50vh">
		<div class="modal-content">
			{#if $base}
				<div class="upstream-commits">
					{#each $base.upstreamCommits.slice(0, 2) as commit, index}
						<CommitCard
							{commit}
							first={index === 0}
							last={(() => {
								if (expanded) {
									return $base.upstreamCommits.length - 1 === index;
								} else {
									if ($base.upstreamCommits.length > 2) {
										return index === 1;
									} else {
										return $base.upstreamCommits.length - 1 === index;
									}
								}
							})()}
							isUnapplied={true}
							commitUrl={$gitHost?.commitUrl(commit.id)}
							type="remote"
							filesToggleable={false}
						/>
					{/each}
					{#if $base.upstreamCommits.length > 2}
						{#if expanded}
							{#each $base.upstreamCommits.slice(2) as commit, index}
								<CommitCard
									{commit}
									last={index === $base.upstreamCommits.length - 3}
									isUnapplied={true}
									commitUrl={$gitHost?.commitUrl(commit.id)}
									type="remote"
									filesToggleable={false}
								/>
							{/each}
							<div class="commit-expand-button">
								<Button wide onclick={() => (expanded = false)}>Hide</Button>
							</div>
						{:else}
							<div class="commit-expand-button">
								<Button wide onclick={() => (expanded = true)}
									>Show more ({$base.upstreamCommits.length - 2})</Button
								>
							</div>
						{/if}
					{/if}
				</div>
			{/if}
			{#if statuses.length > 0}
				<div class="statuses">
					{#each statuses as { branch, status }}
						<div class="branch-status" class:integrated={status.type === 'fullyIntegrated'}>
							<div class="description">
								<h5 class="text-16">{branch?.name || 'Unknown'}</h5>
								{#if status.type === 'conflicted'}
									<p>Conflicted</p>
								{:else if status.type === 'saflyUpdatable' || status.type === 'empty'}
									<p>No Conflicts</p>
								{:else if status.type === 'fullyIntegrated'}
									<p>Integrated</p>
								{/if}
							</div>

							<div class="action" class:action--centered={status.type === 'fullyIntegrated'}>
								{#if status.type === 'fullyIntegrated'}
									<p>Changes included in base branch</p>
								{:else if results.get(branch.id)}
									<Select
										value={results.get(branch.id)!.approach.type}
										onselect={(value) => {
										const result = results.get(branch.id)!

										results.set(branch.id, {...result, approach: { type: value as "rebase" | "merge" | "unapply" }})
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
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</ScrollableContainer>

	{#snippet controls()}
		<Button onclick={() => modal?.close()}>Cancel</Button>
		<Button onclick={integrate} style="pop" kind="solid" loading={integratingUpstream === 'loading'}
			>Integrate</Button
		>
	{/snippet}
</Modal>

<style>
	.upstream-commits {
		text-align: left;
		padding: 0 16px;
	}

	.modal-content {
		display: flex;
		flex-direction: column;
		gap: 14px;
		padding-bottom: 14px;
	}

	.statuses {
		box-sizing: border-box;
		display: flex;
		flex-direction: column;
		justify-content: space-around;
		gap: 14px;
	}

	.branch-status {
		display: flex;
		justify-content: space-between;
		padding: 0 14px;

		&.integrated {
			background-color: var(--clr-bg-2);
		}

		& .description {
			display: flex;
			flex-direction: column;

			gap: 8px;
			text-align: left;
		}

		& .action {
			width: 144px;

			&.action--centered {
				display: flex;
				align-items: center;
				justify-content: center;
			}
		}
	}

	.commit-expand-button {
		margin: 8px -16px;
		padding: 0 16px;
		padding-bottom: 8px;

		border-bottom: 1px solid var(--clr-border-2);
	}
</style>
