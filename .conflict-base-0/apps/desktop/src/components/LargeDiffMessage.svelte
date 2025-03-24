<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';

	interface Props {
		showFrame?: boolean;
		handleShow: () => void;
	}

	const { handleShow, showFrame = false }: Props = $props();

	function show() {
		handleShow();
	}
</script>

<div class="large-diff-message" class:frame-box={showFrame}>
	<p class="text-13">Change hidden as large diffs may slow down the UI</p>
	<Button kind="outline" onclick={show}>Show anyways</Button>
</div>

<style>
	.large-diff-message {
		display: flex;
		padding: 12px;
		gap: 12px;
		flex-direction: column;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.frame-box {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
