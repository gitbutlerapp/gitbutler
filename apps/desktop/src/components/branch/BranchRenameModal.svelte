<script lang="ts" module>
	export type BranchRenameModalProps = {
		projectId: string;
		stackId?: string;
		laneId: string;
		branchName: string;
		isPushed: boolean;
	};
</script>

<script lang="ts">
	import BranchNameTextbox from "$components/branch/BranchNameTextbox.svelte";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Button, ElementId, Modal, TestId } from "@gitbutler/ui";

	const { projectId, laneId, branchName, isPushed }: BranchRenameModalProps = $props();
	const stackService = inject(STACK_SERVICE);

	const [branchRename, branchRenameQuery] = stackService.branchRename;

	let newName: string | undefined = $state();
	let normalizedRefName: string | undefined = $state();
	let isBranchNameValid = $state(false);
	let modal: Modal | undefined = $state();

	let branchNameInput = $state<ReturnType<typeof BranchNameTextbox>>();

	export async function show() {
		newName = branchName;
		modal?.show();
		// Select text after async value is set
		await branchNameInput?.selectAll();
	}
</script>

<Modal
	testId={TestId.BranchHeaderRenameModal}
	width="small"
	title={isPushed ? "Branch has already been pushed" : "Rename branch"}
	type={isPushed ? "warning" : "info"}
	bind:this={modal}
	onSubmit={async (close) => {
		if (normalizedRefName) {
			// The backend re-normalizes; we pass the client-normalized name so the optimistic
			// selection update lands on the name the branch will actually have.
			await branchRename({
				projectId,
				refName: [...new TextEncoder().encode(`refs/heads/${branchName}`)],
				newName: normalizedRefName,
				laneId,
				branchName,
			});
		}
		close();
	}}
>
	<BranchNameTextbox
		bind:this={branchNameInput}
		placeholder="New name"
		id={ElementId.NewBranchNameInput}
		bind:value={newName}
		autofocus
		onnormalizedvalue={(value) => (normalizedRefName = value)}
		onvalidationchange={(isValid) => (isBranchNameValid = isValid)}
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
			disabled={!isBranchNameValid}
			loading={branchRenameQuery.current.isLoading}>Rename</Button
		>
	{/snippet}
</Modal>

<style lang="postcss">
	.helper-text {
		margin-top: 1rem;
		color: var(--text-2);
		line-height: 1.5;
	}
</style>
