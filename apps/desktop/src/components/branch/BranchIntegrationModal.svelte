<script lang="ts">
	import BranchIntegrationModalContent from "$components/branch/BranchIntegrationModalContent.svelte";
	import { Modal, TestId } from "@gitbutler/ui-svelte";

	type Props = {
		modalRef: Modal | undefined;
		projectId: string;
		branchRef: string;
		branchName: string;
	};

	let { modalRef = $bindable(), projectId, branchRef, branchName }: Props = $props();
</script>

<Modal
	bind:this={modalRef}
	title="Update local branch"
	noPadding
	width={720}
	testId={TestId.BranchIntegrationModal}
>
	<BranchIntegrationModalContent
		{projectId}
		{branchRef}
		{branchName}
		closeModal={() => modalRef?.close()}
	/>
</Modal>
