<script lang="ts" module>
	export type CreateSnapshotModalProps = {
		projectId: string;
	};
</script>

<script lang="ts">
	import { HISTORY_SERVICE } from '$lib/history/history';
	import { inject } from '@gitbutler/core/context';
	import { Button, ElementId, Modal, TestId, Textbox } from '@gitbutler/ui';

	const { projectId }: CreateSnapshotModalProps = $props();

	const historyService = inject(HISTORY_SERVICE);

	let message: string = $state('');
	let modal: Modal | undefined = $state();
	let isCreating = $state(false);

	export function show() {
		message = '';
		modal?.show();
	}

	async function createSnapshot(close: () => void) {
		if (isCreating) return;

		try {
			isCreating = true;
			await historyService.createSnapshot(projectId, message || undefined);
			close();
		} catch (error) {
			console.error('Failed to create snapshot:', error);
		} finally {
			isCreating = false;
		}
	}
</script>

<Modal
	testId={TestId.CreateSnapshotModal}
	width="small"
	title="Create snapshot"
	type="info"
	bind:this={modal}
	onSubmit={createSnapshot}
>
	<Textbox
		placeholder="Snapshot description (optional)"
		id={ElementId.SnapshotDescriptionInput}
		bind:value={message}
		autofocus
		helperText="Describe what you're saving for easy reference later"
	/>

	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button
			testId={TestId.CreateSnapshotModal_ActionButton}
			style="pop"
			type="submit"
			loading={isCreating}
		>
			Create snapshot
		</Button>
	{/snippet}
</Modal>
