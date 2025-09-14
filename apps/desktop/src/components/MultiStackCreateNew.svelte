<script lang="ts">
	import CreateBranchModal from '$components/CreateBranchModal.svelte';
	import { Button, TestId } from '@gitbutler/ui';

	type Props = {
		el?: HTMLButtonElement;
		projectId: string;
		stackId?: string;
	};

	let { projectId, stackId }: Props = $props();

	let createBranchModal = $state<CreateBranchModal>();
</script>

<div class="multi-stack-create-new">
	<div class="multi-stack-create-new__button-wrap">
		<Button
			type="button"
			onclick={() => createBranchModal?.show()}
			testId={TestId.CreateStackButton}
			kind="outline"
			icon="plus-small"
		>
			New branch
		</Button>
	</div>
</div>

<CreateBranchModal bind:this={createBranchModal} {projectId} {stackId} />

<style lang="postcss">
	.multi-stack-create-new {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
	}

	.multi-stack-create-new__button-wrap {
		display: flex;
		border-radius: var(--radius-btn);
		background-color: var(--clr-bg-2);
	}
</style>
