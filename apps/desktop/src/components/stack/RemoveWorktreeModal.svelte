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
	let removing = $state(false);

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
		// A second submit while the first is pending would fail: the worktree is gone by then.
		if (removing) return;
		removing = true;
		try {
			await stackService.removeWorktree({ projectId, name, force: true });
			close();
		} finally {
			removing = false;
		}
	}}
>
	<p class="text-13 text-body">
		Remove <span class="text-bold">{name}</span>? Its folder is deleted from disk, along with any
		uncommitted changes in it. Its branch and commits are kept.
	</p>

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="danger" type="submit" loading={removing}>Remove</Button>
	{/snippet}
</Modal>
