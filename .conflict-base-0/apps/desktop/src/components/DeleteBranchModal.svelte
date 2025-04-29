<script lang="ts">
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();
	const [stackService] = inject(StackService);
	const [removeBranch, branchRemovalOp] = stackService.removeBranch;

	let modal = $state<Modal>();

	export function show() {
		modal?.show();
	}
</script>

<Modal
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
	{#snippet children()}
		Are you sure you want to delete <code class="code-string">{branchName}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" loading={branchRemovalOp.current.isLoading}>Delete</Button>
	{/snippet}
</Modal>
