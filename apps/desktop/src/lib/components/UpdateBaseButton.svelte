<script lang="ts">
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import CommitCard from '$lib/commit/CommitCard.svelte';
	import { getGitHost } from '$lib/gitHost/interface/gitHost';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import { getContext } from '$lib/utils/context';
	import {
		UpstreamIntegrationService,
		type BranchStatusesWithBranches
	} from '$lib/vbranches/upstreamIntegrationService';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import type { Readable } from 'svelte/store';

	const upstreamIntegrationService = getContext(UpstreamIntegrationService);
	const baseBranchService = getContext(BaseBranchService);
	const gitHost = getGitHost();

	const base = baseBranchService.base;

	let modal = $state<Modal>();

	let modalOpeningState = $state<'inert' | 'loading' | 'completed'>('inert');
	let branchStatuses = $state<Readable<BranchStatusesWithBranches | undefined>>();

	const statuses = $derived.by(() => {
		if ($branchStatuses?.type !== 'UpdatesRequired') return [];

		const statuses = [...$branchStatuses.subject];
		statuses.sort((a, b) => {
			if (
				(a.status.type !== 'FullyIntegrated' && b.status.type !== 'FullyIntegrated') ||
				(a.status.type === 'FullyIntegrated' && b.status.type === 'FullyIntegrated')
			) {
				return (a.branch?.name || 'Unknown').localeCompare(b.branch?.name || 'Unknown');
			}

			if (a.status.type === 'FullyIntegrated') {
				return 1;
			} else {
				return -1;
			}
		});

		return statuses;
	});

	$effect(() => {
		if ($branchStatuses && modalOpeningState === 'loading') {
			modalOpeningState = 'completed';
			modal?.show();
			console.log(modalOpeningState);
		}
	});

	function openModal() {
		modalOpeningState = 'loading';
		branchStatuses = upstreamIntegrationService.upstreamStatuses();
	}

	function onClose() {
		modalOpeningState = 'inert';
	}

	$inspect($branchStatuses);
</script>

<Modal bind:this={modal} title="Integrate upstream changes" {onClose} width="small">
	{#if $base}
		<div class="upstream-commits">
			{#each $base.upstreamCommits as commit, index}
				<CommitCard
					{commit}
					first={index === 0}
					last={index === $base.upstreamCommits.length - 1}
					isUnapplied={true}
					commitUrl={$gitHost?.commitUrl(commit.id)}
					type="remote"
				/>
			{/each}
		</div>
	{/if}
	<div class="statuses">
		{#each statuses as { branch, status }}
			<div class="branch-status" class:integrated={status.type === 'FullyIntegrated'}>
				<div class="description">
					<h5 class="text-16">{branch?.name || 'Unknown'}</h5>
					{#if status.type === 'Conflicted'}
						<p>Conflicted</p>
					{:else if status.type === 'SaflyUpdatable' || status.type === 'Empty'}
						<p>No Conflicts</p>
					{:else if status.type === 'FullyIntegrated'}
						<p>Integrated</p>
					{/if}
				</div>

				<div class="action" class:action--centered={status.type === 'FullyIntegrated'}>
					{#if status.type === 'FullyIntegrated'}
						<p>Will be liberated</p>
					{:else}
						<Select
							value="rebase"
							options={[
								{ label: 'Rebase', value: 'rebase' },
								{ label: 'Merge', value: 'merge' },
								{ label: 'Stash', value: 'stash' }
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

	{#snippet controls()}
		<Button onclick={() => modal?.close()}>Cancel</Button>
		<Button onclick={() => modal?.close()} style="pop" kind="solid">Integrate</Button>
	{/snippet}
</Modal>

<Button
	size="tag"
	style="error"
	kind="solid"
	tooltip="Merge upstream into common base"
	onclick={openModal}
	loading={modalOpeningState === 'loading'}
>
	Update
</Button>

<style>
	.upstream-commits {
		text-align: left;

		margin-top: -10px;
		margin-bottom: 8px;
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
</style>
