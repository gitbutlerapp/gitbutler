<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import { type Snippet } from 'svelte';

	interface Props {
		CancelButtonLabel?: string;
		onCancel: () => void;
		children: Snippet;
	}

	const { children, CancelButtonLabel = 'Cancel', onCancel }: Props = $props();
</script>

<div class="editor-footer">
	<Button kind="outline" style="neutral" onclick={onCancel}>{CancelButtonLabel}</Button>
	{@render children()}
</div>

<style lang="postcss">
	.editor-footer {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 6px;
		margin-top: 14px;
	}
</style>
