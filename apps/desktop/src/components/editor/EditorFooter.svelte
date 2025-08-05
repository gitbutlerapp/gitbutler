<script lang="ts">
	import { Button, TestId } from '@gitbutler/ui';
	import { type Snippet } from 'svelte';

	interface Props {
		CancelButtonLabel?: string;
		onCancel: () => void;
		children: Snippet;
	}

	const { children, CancelButtonLabel = 'Cancel', onCancel }: Props = $props();
</script>

<div class="editor-footer">
	<Button
		testId={TestId.CommitDrawerCancelButton}
		kind="outline"
		width={120}
		style="neutral"
		onclick={onCancel}>{CancelButtonLabel}</Button
	>
	{@render children()}
</div>

<style lang="postcss">
	.editor-footer {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		margin-top: 14px;
		gap: 6px;
	}
</style>
