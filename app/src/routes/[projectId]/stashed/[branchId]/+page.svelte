<script lang="ts">
	// This page is displayed when:
	// - A vbranch is found
	// It may also display details about a cooresponding remote and/or pr if they exist
	import BranchLane from '$lib/branch/BranchLane.svelte';
	import FullviewLoading from '$lib/components/FullviewLoading.svelte';
	import Button from '$lib/shared/Button.svelte';
	import Modal from '$lib/shared/Modal.svelte';
	import type { PageData } from './$types';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	export let data: PageData;

	let applyConflictedModal: Modal;
	let deleteBranchModal: Modal;

	$: ({ projectId, branchController, vbranchService } = data);
	$: branches$ = vbranchService.branches$;
	$: error = vbranchService.branchesError;

	$: branch = $branches$?.find((b) => b.id === $page.params.branchId);
</script>

{#if $error}
	<p>{JSON.stringify($error)}</p>
{:else if !$branches$}
	<FullviewLoading />
{:else if branch}
	<BranchLane {branch} isUnapplied={!branch.active} />
{:else}
	<p>Branch no longer exists</p>
{/if}

<Modal width="small" title="Merge conflicts" bind:this={applyConflictedModal}>
	<p>Applying this branch will introduce merge conflicts.</p>
	{#snippet controls(close, item)}
		<Button style="ghost" outline on:click={close}>Cancel</Button>
		<Button
			style="pop"
			kind="solid"
			on:click={() => {
				branchController.applyBranch(item.id);
				close();
				goto(`/${projectId}/board`);
			}}
		>
			Update
		</Button>
	{/snippet}
</Modal>

<Modal width="small" title="Delete branch" bind:this={deleteBranchModal}>
	{#snippet children(item)}
		Deleting <code class="code-string">{item.name}</code> cannot be undone.
	{/snippet}
	{#snippet controls(close, item)}
		<Button style="ghost" outline on:mousedown={close}>Cancel</Button>
		<Button
			style="error"
			kind="solid"
			on:click={() => {
				branchController.deleteBranch(item.id);
				close();
				goto(`/${projectId}/board`);
			}}
		>
			Delete
		</Button>
	{/snippet}
</Modal>
