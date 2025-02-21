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
	<Button kind="outline" style="neutral" onclick={onCancel} width={94}>{CancelButtonLabel}</Button>
	{@render children()}
</div>

<style lang="postcss">
	.editor-footer {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 16px;
		border-top: 1px solid var(--clr-border-2);
	}
</style>
