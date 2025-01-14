<script lang="ts">
	import IntegrateUpstreamModal from './IntegrateUpstreamModal.svelte';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const baseBranchService = getContext(BaseBranchService);

	const base = baseBranchService.base;

	const displayButton = $derived.by(() => {
		const hasUpstreamCommits = ($base?.upstreamCommits?.length ?? 0) > 0;
		const diverged = $base?.diverged ?? true;
		return hasUpstreamCommits && !diverged;
	});

	let modal = $state<ReturnType<typeof IntegrateUpstreamModal>>();

	function openModal() {
		modal?.show();
	}
</script>

<IntegrateUpstreamModal bind:this={modal} />

{#if displayButton}
	<Button
		size="tag"
		style="error"
		tooltip="Merge upstream into common base"
		onclick={() => {
			openModal();
		}}
		loading={modal?.imports.open}
	>
		Update
	</Button>
{/if}
