<script lang="ts">
	import { persisted } from '@gitbutler/shared/persisted';

	import {
		Button,
		ContextMenuItem,
		ContextMenuSection,
		DropDownButton,
		TestId
	} from '@gitbutler/ui';

	interface Props {
		isSubmitting: boolean;
		canPublishPR: boolean;
		submitDisabled?: boolean;
		reviewUnit: string | undefined;
		onCancel: () => void;
		onSubmit: () => void;
	}

	let { canPublishPR, submitDisabled, isSubmitting, onCancel, onSubmit, reviewUnit }: Props =
		$props();

	const unit = $derived(reviewUnit ?? 'PR');
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
		{$createDraft ? `Create ${unit} draft` : `Create ${unit}`}

		{#snippet contextMenuSlot()}
			<ContextMenuSection>
				<ContextMenuItem
					label="Create {unit} draft"
					onclick={() => {
						$createDraft = true;
						commitButton?.close();
					}}
					disabled={!canPublishPR}
				/>

				<ContextMenuItem
					label="Create {unit}"
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
