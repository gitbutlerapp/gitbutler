<script lang="ts">
	import { BranchStack } from '$lib/branches/branch';
	import { BranchController } from '$lib/branches/branchController';
	import { openExternalUrl } from '$lib/utils/url';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import LinkButton from '@gitbutler/ui/LinkButton.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { error } from '@gitbutler/ui/toasts';
	import { slugify } from '@gitbutler/ui/utils/string';

	interface Props {
		parentSeriesName: string;
	}

	const BRANCH_STACKING_DOCS = 'https://docs.gitbutler.com/features/stacked-branches';
	function clickOnDocsLink() {
		openExternalUrl(BRANCH_STACKING_DOCS);
	}

	const { parentSeriesName }: Props = $props();

	const branchController = getContext(BranchController);
	const stack = getContextStore(BranchStack);

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

		branchController.createPatchSeries($stack.id, slugifiedRefName);
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
		<Textbox
			label="Branch name"
			id="newRemoteName"
			bind:value={createRefName}
			autofocus
			helperText={generatedNameDiverges ? `Will be created as '${slugifiedRefName}'` : undefined}
		/>

		<p class="text-12 text-body helper-text">
			Creates a new branch that depends on <strong>{parentSeriesName}</strong>. The branches will
			have to be reviewed and merged in order. Learn more about stacked branches in the
			<LinkButton onclick={clickOnDocsLink}>
				{#snippet children()}
					docs
				{/snippet}
			</LinkButton>.
		</p>
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit" disabled={!createRefName}>Add new branch</Button>
	{/snippet}
</Modal>

<style>
	.helper-text {
		color: var(--clr-scale-ntrl-50);
		margin-top: 10px;
	}
</style>
