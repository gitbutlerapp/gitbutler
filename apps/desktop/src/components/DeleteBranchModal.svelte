<script lang="ts" module>
	export type DeleteBranchModalProps = {
		projectId: string;
		stackId?: string;
		branchName: string;
	};
</script>

<script lang="ts">
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Button, Modal, TestId } from '@gitbutler/ui';

	const { projectId, stackId, branchName }: DeleteBranchModalProps = $props();
	const stackService = inject(STACK_SERVICE);
	const [removeBranch, branchRemovalOp] = stackService.removeBranch;

	let modal = $state<Modal>();

	export function show() {
		modal?.show();
	}
</script>

<Modal
	testId={TestId.BranchHeaderDeleteModal}
	bind:this={modal}
	width="small"
	title="Delete branch"
	onSubmit={async (close) => {
		await removeBranch({
			projectId,
			stackId,
			branchName
		});
		close();
	}}
>
	<p class="text-13 text-body">
		Are you sure you want to delete <code class="code-string">{branchName}</code>?
	</p>
	{#snippet controls(close)}
		<Button kind="outline" onclick={close} autofocus>Cancel</Button>
		<Button
			testId={TestId.BranchHeaderDeleteModal_ActionButton}
			style="danger"
			type="submit"
			loading={branchRemovalOp.current.isLoading}>Delete</Button
		>
	{/snippet}
</Modal>
