<script lang="ts" module>
	export type BranchRenameModalProps = {
		projectId: string;
		stackId: string;
		branchName: string;
		isPushed: boolean;
	};
</script>

<script lang="ts">
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button, ElementId, Modal, TestId, Textbox } from '@gitbutler/ui';

	const { projectId, stackId, branchName, isPushed }: BranchRenameModalProps = $props();
	const stackService = inject(STACK_SERVICE);

	const [renameBranch, renameResult] = stackService.updateBranchName;

	let newName: string | undefined = $state();
	let modal: Modal | undefined = $state();

	export function show() {
		newName = branchName;
		modal?.show();
	}
</script>

<Modal
	testId={TestId.BranchHeaderRenameModal}
	width="small"
	title={isPushed ? 'Branch has already been pushed' : 'Rename branch'}
	type={isPushed ? 'warning' : 'info'}
	bind:this={modal}
	onSubmit={async (close) => {
		if (newName) {
			renameBranch({ projectId, stackId, branchName, newName });
		}
		close();
	}}
>
	<Textbox
		placeholder="New name"
		id={ElementId.NewBranchNameInput}
		bind:value={newName}
		autofocus
	/>

	{#if isPushed}
		<div class="text-12 helper-text">
			Renaming a branch that has already been pushed will create a new branch at the remote. The old
			one will remain untouched but will be disassociated from this branch.
		</div>
	{/if}

	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button
			testId={TestId.BranchHeaderRenameModal_ActionButton}
			style="pop"
			type="submit"
			disabled={!newName}
			loading={renameResult.current.isLoading}>Rename</Button
		>
	{/snippet}
</Modal>

<style lang="postcss">
	.helper-text {
		margin-top: 1rem;
		color: var(--clr-scale-ntrl-50);
		line-height: 1.5;
	}
</style>
