<script lang="ts">
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, Modal } from "@gitbutler/ui-svelte";

	type Props = {
		projectId: string;
		name: string;
	};

	const { projectId, name }: Props = $props();

	const stackService = inject(STACK_SERVICE);

	let modal = $state<Modal>();

	export function show() {
		modal?.show();
	}
</script>

<Modal
	bind:this={modal}
	width="small"
	type="warning"
	title="Remove worktree"
	onSubmit={async (close) => {
		await stackService.removeWorktree({ projectId, name, force: true });
		close();
	}}
>
	<p class="text-13 text-body">
		Remove <span class="text-bold">{name}</span>? Its folder is deleted from disk, along with any
		uncommitted changes in it. Its branch and commits are kept.
	</p>

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="danger" type="submit">Remove</Button>
	{/snippet}
</Modal>
