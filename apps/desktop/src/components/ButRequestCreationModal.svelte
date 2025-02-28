<script lang="ts">
	import { BranchStack } from '$lib/branches/branch';
	import { BranchController } from '$lib/branches/branchController';
	import { ButRequestDetailsService } from '$lib/forge/butRequestDetailsService';
	import { StackPublishingService } from '$lib/history/stackPublishingService';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	type Props = {
		branchTitle: string;
	};

	const { branchTitle }: Props = $props();

	const stack = getContextStore(BranchStack);
	const stackPublishingService = getContext(StackPublishingService);
	const branchController = getContext(BranchController);
	const butRequestDetailsService = getContext(ButRequestDetailsService);

	let modal = $state<Modal>();

	let title = $state(branchTitle);
	let description = $state('');

	async function publishReview() {
		await branchController.pushBranch($stack.id, true);
		const reviewId = await stackPublishingService.upsertStack($stack.id, branchTitle);
		console.log(reviewId);
		butRequestDetailsService.setDetails(reviewId, title, description);
		modal?.close();
	}

	export function show() {
		modal?.show();
	}
	export function close() {
		modal?.close();
	}
</script>

<Modal bind:this={modal} title="Create a Butler Request">
	{#snippet children()}
		<div class="container">
			<Textbox bind:value={title} placeholder="Add title..." />
			<Textarea bind:value={description} placeholder="Add description..." />
		</div>
	{/snippet}

	{#snippet controls()}
		<Button onclick={() => modal?.close()} kind="outline">Cancel</Button>
		<AsyncButton action={publishReview} style="pop">Publish Butler Request</AsyncButton>
	{/snippet}
</Modal>

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}
</style>
