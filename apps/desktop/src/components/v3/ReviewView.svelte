<script lang="ts">
	import ReviewCreation from '$components/ReviewCreation.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
	};

	const { projectId, stackId, branchName }: Props = $props();

	const uiState = getContext(UiState);

	let drawer = $state<ReturnType<typeof Drawer>>();
	let reviewCreation = $state<ReviewCreation>();

	function close() {
		uiState.project(projectId).drawerPage.current = 'branch';
	}
</script>

<Drawer bind:this={drawer} {projectId} {stackId} title="Submit for code review">
	<div class="container">
		<div class="main">
			<ReviewCreation
				bind:this={reviewCreation}
				{projectId}
				{stackId}
				{branchName}
				onClose={close}
			/>
		</div>

		<div class="actions">
			<Button kind="outline" onclick={close}>Cancel</Button>
			<AsyncButton
				action={async () => await reviewCreation?.createReview()}
				loading={reviewCreation?.imports.isLoading}
				disabled={!reviewCreation?.createButtonEnabled().current}>Create Review</AsyncButton
			>
		</div>
	</div>
</Drawer>

<style lang="postcss">
	.container {
		flex-grow: 1;

		display: flex;
		flex-direction: column;
	}

	.main {
		flex-grow: 1;
	}

	.actions {
		display: flex;
		justify-content: flex-end;

		gap: 12px;

		width: 100%;

		margin-top: 14px;
	}
</style>
