<script lang="ts">
	import TextBox from '$lib/shared/TextBox.svelte';
	import { slugify } from '$lib/utils/string';
	import { error } from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

	interface Props {
		parentSeriesName: string;
	}

	const { parentSeriesName }: Props = $props();

	const branchController = getContext(BranchController);
	const branch = getContextStore(VirtualBranch);

	let createRefModal = $state<ReturnType<typeof Modal>>();
	let createRefName: string | undefined = $state();
	const slugifiedRefName = $derived(createRefName && slugify(createRefName));
	const generatedNameDiverges = $derived(!!createRefName && slugifiedRefName !== createRefName);

	function addSeries() {
		if (!slugifiedRefName) {
			error('No branch name provided');
			createRefModal?.close();
			return;
		}

		branchController.createPatchSeries($branch.id, slugifiedRefName);
		createRefModal?.close();
	}

	function onModalClose() {
		createRefName = undefined;
	}

	export function show() {
		createRefModal?.show();
	}
</script>

<Modal
	bind:this={createRefModal}
	title="Add branch to the stack"
	width="small"
	onSubmit={addSeries}
	onClose={onModalClose}
>
	{#snippet children()}
		<TextBox
			label="Branch name"
			id="newRemoteName"
			bind:value={createRefName}
			focus
			helperText={generatedNameDiverges ? `Will be created as '${slugifiedRefName}'` : undefined}
		/>

		<p class="text-12 text-body helper-text">
			Creates a new branch that depends on <strong>{parentSeriesName}</strong>. The branches will
			have to be reviewed and merged in order.
		</p>
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit" kind="solid" disabled={!createRefName}>Add new branch</Button>
	{/snippet}
</Modal>

<style>
	.helper-text {
		color: var(--clr-scale-ntrl-50);
		margin-top: 10px;
	}
</style>
