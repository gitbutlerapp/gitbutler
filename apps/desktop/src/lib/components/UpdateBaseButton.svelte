<script lang="ts">
	import IntegrateUpstreamModal from './IntegrateUpstreamModal.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { showInfo } from '$lib/notifications/toasts';
	import { getContext } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import Button from '@gitbutler/ui/Button.svelte';

	const baseBranchService = getContext(BaseBranchService);
	const branchController = getContext(BranchController);
	const project = getContext(Project);

	const base = baseBranchService.base;

	const displayButton = $derived.by(() => {
		const hasUpstreamCommits = ($base?.upstreamCommits?.length ?? 0) > 0;
		return hasUpstreamCommits;
	});

	let modal = $state<IntegrateUpstreamModal>();

	function openModal() {
		modal?.show();
	}

	async function updateBaseBranch() {
		let infoText = await branchController.updateBaseBranch();
		if (infoText) {
			showInfo('Stashed conflicting branches', infoText);
		}
	}
</script>

<IntegrateUpstreamModal bind:this={modal} />

{#if displayButton}
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
		loading={modal?.imports.open}
	>
		Update
	</Button>
{/if}
