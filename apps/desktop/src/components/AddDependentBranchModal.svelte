<script lang="ts" module>
	export type AddDependentBranchModalProps = {
		projectId: string;
		stackId: string;
	};
</script>

<script lang="ts">
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import { Button, Modal, Textbox, TestId } from '@gitbutler/ui';
	import { slugify } from '@gitbutler/ui/utils/string';

	const { projectId, stackId }: AddDependentBranchModalProps = $props();

	const stackService = inject(STACK_SERVICE);
	const [createNewBranch, branchCreation] = stackService.newBranch;

	let modal = $state<Modal>();
	let branchName = $state<string>();

	const slugifiedRefName = $derived(branchName && slugify(branchName));
	const generatedNameDiverges = $derived(!!branchName && slugifiedRefName !== branchName);

	async function handleAddDependentBranch(close: () => void) {
		if (!slugifiedRefName) return;

		await createNewBranch({
			projectId,
			stackId,
			request: {
				targetPatch: undefined,
				name: slugifiedRefName
			}
		});

		close();
	}

	export function show() {
		modal?.show();
	}
</script>

<Modal
	testId={TestId.BranchHeaderAddDependanttBranchModal}
	bind:this={modal}
	width="small"
	title="Add dependent branch"
	onSubmit={handleAddDependentBranch}
>
	<div class="content-wrap">
		<Textbox
			placeholder="Branch name"
			bind:value={branchName}
			autofocus
			helperText={generatedNameDiverges ? `Will be created as '${slugifiedRefName}'` : undefined}
		/>
	</div>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button
			testId={TestId.BranchHeaderAddDependanttBranchModal_ActionButton}
			style="pop"
			type="submit"
			disabled={!slugifiedRefName}
			loading={branchCreation.current.isLoading}>Add branch</Button
		>
	{/snippet}
</Modal>
