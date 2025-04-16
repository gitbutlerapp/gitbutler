<script lang="ts">
	import { persisted } from '@gitbutler/shared/persisted';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';

	interface Props {
		isSubmitting: boolean;
		canPublishBR: boolean;
		canPublishPR: boolean;
		ctaDisabled: boolean;
		onCancel: () => void;
		onSubmit: () => void;
	}

	let { canPublishBR, canPublishPR, ctaDisabled, isSubmitting, onCancel, onSubmit }: Props =
		$props();

	const createDraft = persisted<boolean>(false, 'createDraftPr');

	function getCtaLabel() {
		if (canPublishBR && canPublishPR) {
			return 'Create review';
		} else if (canPublishBR) {
			return 'Create Butler Request';
		} else if (canPublishPR) {
			return 'Create Pull Request';
		}

		return 'Create review';
	}
</script>

<div class="submit-review-actions">
	<div class="submit-review-actions__extra">
		{#if canPublishPR && !canPublishBR}
			<label for="create-pr-draft" class="submit-review-actions__drafty">
				<Toggle id="create-pr-draft" bind:checked={$createDraft} disabled={isSubmitting} />
				<span class="text-13">PR draft</span>
			</label>
		{/if}
	</div>

	<div class="submit-review-actions__general">
		<Button kind="outline" loading={isSubmitting} onclick={onCancel}>Cancel</Button>
		<AsyncButton
			width={166}
			style="pop"
			action={async () => onSubmit()}
			disabled={ctaDisabled}
			loading={isSubmitting}>{getCtaLabel()}</AsyncButton
		>
	</div>
</div>

<style lang="postcss">
	.submit-review-actions {
		display: flex;
		justify-content: space-between;
		gap: 6px;
		width: 100%;
	}

	.submit-review-actions__extra,
	.submit-review-actions__general {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.submit-review-actions__drafty {
		display: flex;
		align-items: center;
		gap: 8px;
	}
</style>
