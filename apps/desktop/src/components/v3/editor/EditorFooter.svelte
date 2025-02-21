<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		SubmitButtonLabel: string;
		CancelButtonLabel?: string;
		onSubmit: () => void;
		onCancel: () => void;
	}

	const { SubmitButtonLabel, CancelButtonLabel = 'Cancel', onSubmit, onCancel }: Props = $props();
</script>

<div class="editor-footer">
	<Button kind="outline" style="neutral" onclick={onCancel} width={94}>{CancelButtonLabel}</Button>
	<Button style="pop" onclick={onSubmit} wide>{SubmitButtonLabel}</Button>
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
