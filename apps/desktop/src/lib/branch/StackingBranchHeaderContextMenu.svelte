<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

	interface Props {
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		target?: HTMLElement;
		headName: string;
		description?: string;
		toggleDescription: () => Promise<void>;
	}

	let {
		contextMenuEl = $bindable(),
		target,
		headName,
		description,
		toggleDescription
	}: Props = $props();

	const branchStore = getContextStore(VirtualBranch);
	const branchController = getContext(BranchController);

	let deleteSeriesModal: Modal;
	let renameSeriesModal: Modal;
	let newHeadName: string = $state(headName);
	let isDeleting = $state(false);
	let showDescription = $state(!!description);

	const branch = $derived($branchStore);
</script>

<ContextMenu bind:this={contextMenuEl} {target}>
	<ContextMenuSection>
		<ContextMenuItem
			label={`${!showDescription ? 'Add' : 'Remove'} description`}
			on:click={async () => {
				await toggleDescription();
				showDescription = !showDescription;
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Rename"
			on:click={async () => {
				renameSeriesModal.show(branch);
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Delete"
			on:click={() => {
				deleteSeriesModal.show(branch);
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

<Modal
	width="small"
	title="Rename series"
	bind:this={renameSeriesModal}
	onSubmit={(close) => {
		if (newHeadName && newHeadName !== headName) {
			branchController.updateSeriesName(branch.id, headName, newHeadName);
		}
		close();
	}}
>
	<TextBox placeholder="New name" id="newSeriesName" bind:value={newHeadName} focus />

	{#snippet controls(close)}
		<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" kind="solid" type="submit">Rename</Button>
	{/snippet}
</Modal>

<Modal
	width="small"
	title="Delete series"
	bind:this={deleteSeriesModal}
	onSubmit={async (close) => {
		try {
			isDeleting = true;
			await branchController.removePatchSeries(branch.id, headName);
			close();
		} finally {
			isDeleting = false;
		}
	}}
>
	{#snippet children(branch)}
		Are you sure you want to delete <code class="code-string">{branch.name}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline onclick={close}>Cancel</Button>
		<Button style="error" kind="solid" type="submit" loading={isDeleting}>Delete</Button>
	{/snippet}
</Modal>
