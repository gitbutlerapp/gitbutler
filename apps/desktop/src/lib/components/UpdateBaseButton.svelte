<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import CommitCard from '$lib/commit/CommitCard.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import { showInfo } from '$lib/notifications/toasts';
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import { getContext } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import {
		UpstreamIntegrationService,
		type BranchStatus,
		type BranchStatusesWithBranches,
		type Resolution,
		type ResolutionApproach
	} from '$lib/vbranches/upstreamIntegrationService';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import type { Readable } from 'svelte/store';

	interface Props {
		showButton?: boolean;
	}

	const { showButton = true }: Props = $props();

	const upstreamIntegrationService = getContext(UpstreamIntegrationService);
	const baseBranchService = getContext(BaseBranchService);
	const gitHost = getGitHost();
	const branchController = getContext(BranchController);
	const project = getContext(Project);

	const base = baseBranchService.base;

	let modal = $state<Modal>();

	let modalOpeningState = $state<'inert' | 'loading' | 'completed'>('inert');
	let branchStatuses = $state<Readable<BranchStatusesWithBranches | undefined>>();

	let results = $state(new SvelteMap<string, Resolution>());

	let statuses = $state<{ branch: VirtualBranch; status: BranchStatus }[]>([]);

	$effect(() => {
		if ($branchStatuses?.type !== 'updatesRequired') {
			statuses = [];
			return;
		}

		const statusesTmp = [...$branchStatuses.subject];
		statusesTmp.sort((a, b) => {
			if (
				(a.status.type !== 'fullyIntegrated' && b.status.type !== 'fullyIntegrated') ||
				(a.status.type === 'fullyIntegrated' && b.status.type === 'fullyIntegrated')
			) {
				return (a.branch?.name || 'Unknown').localeCompare(b.branch?.name || 'Unknown');
			}

			if (a.status.type === 'fullyIntegrated') {
				return 1;
			} else {
				return -1;
			}
		});

		// Side effect, refresh results
		results = new SvelteMap(
			statusesTmp.map((status) => {
				let defaultApproach: ResolutionApproach;

				if (status.status.type === 'fullyIntegrated') {
					defaultApproach = { type: 'delete' };
				} else {
					if (status.branch.allowRebasing) {
						defaultApproach = { type: 'rebase' };
					} else {
						defaultApproach = { type: 'merge' };
					}
				}

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

	$effect(() => {
		if ($branchStatuses && modalOpeningState === 'loading') {
			modalOpeningState = 'completed';
			modal?.show();
			console.log(modalOpeningState);
		}
	});

	let integratingUpstream = $state<'inert' | 'loading' | 'complete'>('inert');

	export function openModal() {
		modalOpeningState = 'loading';
		integratingUpstream = 'inert';
		expanded = false;
		branchStatuses = upstreamIntegrationService.upstreamStatuses();
	}

	function onClose() {
		modalOpeningState = 'inert';
	}

	async function integrate() {
		integratingUpstream = 'loading';
		await upstreamIntegrationService.integrateUpstream([...results.values()]);
		await baseBranchService.refresh();
		integratingUpstream = 'complete';

		modal?.close();
	}

	async function updateBaseBranch() {
		let infoText = await branchController.updateBaseBranch();
		if (infoText) {
			showInfo('Stashed conflicting branches', infoText);
		}
	}

	let expanded = $state(false);
</script>

<Modal bind:this={modal} title="Integrate upstream changes" {onClose} width="small" noPadding>
	<ScrollableContainer maxHeight="50vh">
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
	</ScrollableContainer>

	{#snippet controls()}
		<Button onclick={() => modal?.close()}>Cancel</Button>
		<Button onclick={integrate} style="pop" kind="solid" loading={integratingUpstream === 'loading'}
			>Integrate</Button
		>
	{/snippet}
</Modal>

{#if showButton && ($base?.upstreamCommits.length || 0) > 0}
	<Button
		size="tag"
		style="error"
		kind="solid"
		tooltip="Merge upstream into common base"
		onclick={() => {
			if (project.succeedingRebases) {
				openModal();
			} else {
				updateBaseBranch();
			}
		}}
		loading={modalOpeningState === 'loading'}
	>
		Update
	</Button>
{/if}

<style>
	.upstream-commits {
		text-align: left;

		padding: 0 16px;
	}

	.branch-status {
		display: flex;
		justify-content: space-between;

		padding: 14px;

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
