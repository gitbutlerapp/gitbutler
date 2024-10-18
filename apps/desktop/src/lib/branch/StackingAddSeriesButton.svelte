<script lang="ts">
	import TextBox from '$lib/shared/TextBox.svelte';
	import { slugify } from '$lib/utils/string';
	import { error } from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
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
</script>

<button class="add-branch-btn text-12" onclick={() => createRefModal?.show()}>
	<span class="add-branch-btn__label"> New dependent branch </span>
	<Icon name="plus-small" />
</button>

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
			helperText={generatedNameDiverges ? `Will be created as '${slugifiedRefName}''` : undefined}
		/>

		<p class="text-12 text-body helper-text">
			Creates a new branch that depends on {parentSeriesName}. The branches will have to be reviewed
			and merged in order.
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
		margin-top: 6px;
	}

	.add-branch-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2px 4px;
		height: var(--size-m);
		background-color: var(--clr-theme-ntrl-element);
		color: var(--clr-theme-ntrl-on-element);
		border-radius: var(--radius-m);

		&:hover {
			background-color: var(--clr-theme-ntrl-element-hover);

			& .add-branch-btn__label {
				max-width: 120px;
				margin-left: 4px;
				margin-right: 3px;
				opacity: 1;
			}
		}
	}

	.add-branch-btn__label {
		overflow: hidden;
		white-space: nowrap;
		max-width: 0;
		opacity: 0;

		transition:
			max-width 0.2s,
			opacity 0.3s;
	}
</style>
