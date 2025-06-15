<script lang="ts">
	import { TestId } from '$lib/testing/testIds';
	import { persisted } from '@gitbutler/shared/persisted';

	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import DropDownButton from '@gitbutler/ui/DropDownButton.svelte';

	interface Props {
		isSubmitting: boolean;
		canPublishPR: boolean;
		submitDisabled: boolean;
		onCancel: () => void;
		onSubmit: () => void;
	}

	let { canPublishPR, submitDisabled, isSubmitting, onCancel, onSubmit }: Props = $props();

	let commitButton = $state<DropDownButton>();

	const createDraft = persisted<boolean>(false, 'createDraftPr');
</script>

<div class="submit-review-actions">
	<Button
		testId={TestId.ReviewCancelButton}
		kind="outline"
		disabled={isSubmitting}
		width={120}
		onclick={onCancel}>Cancel</Button
	>

	<DropDownButton
		testId={TestId.ReviewCreateButton}
		bind:this={commitButton}
		onclick={() => {
			if (isSubmitting) return;
			onSubmit();
		}}
		wide
		style="pop"
		loading={isSubmitting}
		disabled={submitDisabled}
	>
		{$createDraft ? 'Create PR draft' : 'Create Pull Request'}

		{#snippet contextMenuSlot()}
			<ContextMenuSection>
				<ContextMenuItem
					label="Create PR draft"
					onclick={() => {
						$createDraft = true;
						commitButton?.close();
					}}
					disabled={!canPublishPR}
				/>

				<ContextMenuItem
					label="Create Pull Request"
					onclick={() => {
						$createDraft = false;
						commitButton?.close();
					}}
				/>
			</ContextMenuSection>
		{/snippet}
	</DropDownButton>
</div>

<style lang="postcss">
	.submit-review-actions {
		display: flex;
		justify-content: flex-end;
		width: 100%;
		gap: 6px;
	}
</style>
