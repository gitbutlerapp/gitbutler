<script lang="ts">
	import IntegrateUpstreamModal from '$components/IntegrateUpstreamModal.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { Project } from '$lib/project/project';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';

	const project = getContext(Project);
	const projectId = $derived(project.id);
	const baseBranchService = getContext(BaseBranchService);
	const baseResponse = $derived(baseBranchService.baseBranch(projectId));
	const base = $derived(baseResponse.current.data);

	const displayButton = $derived.by(() => {
		const hasUpstreamCommits = (base?.upstreamCommits?.length ?? 0) > 0;
		const diverged = base?.diverged ?? true;
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
