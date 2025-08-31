<script lang="ts">
	import InteractiveBranchIntegration from '$components/InteractiveBranchIntegration.svelte';
	import { Modal } from '@gitbutler/ui';
	type Props = {
		modalRef: Modal | undefined;
		projectId: string;
		stackId: string | undefined;
		branchName: string;
	};

	let { modalRef = $bindable(), projectId, stackId, branchName }: Props = $props();

	function closeModal() {
		modalRef?.close();
	}
</script>

<Modal bind:this={modalRef} title="Integrate the upstream changes" noPadding width={500}>
	<InteractiveBranchIntegration {projectId} {stackId} {branchName} {closeModal} />
</Modal>
