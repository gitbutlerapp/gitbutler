<script lang="ts">
	import BranchIntegrationModal from "$components/branch/BranchIntegrationModal.svelte";
	import { Button, Modal, TestId } from "@gitbutler/ui";

	type Props = {
		projectId: string;
		stackId: string | undefined;
		branchName: string;
		branchRef: string;
	};

	const { projectId, branchName, branchRef }: Props = $props();

	let integrationModal = $state<Modal>();

	function kickOffIntegration() {
		integrationModal?.show();
	}
</script>

<BranchIntegrationModal bind:modalRef={integrationModal} {projectId} {branchName} {branchRef} />

<form
	class="upstream-integration-actions"
	onsubmit={(e) => {
		e.preventDefault();
		kickOffIntegration();
	}}
>
	<Button
		type="submit"
		style="warning"
		kind="outline"
		testId={TestId.UpstreamCommitsIntegrateButton}
	>
		Update local branch...
	</Button>
</form>

<style lang="postcss">
	.upstream-integration-actions {
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
</style>
