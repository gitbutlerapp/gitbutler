<script lang="ts">
	import { Modal, Button } from '@gitbutler/ui';

	type Props = {
		fileName: string;
		onConfirm: () => void;
		onCancel: () => void;
	};

	const { fileName, onConfirm, onCancel }: Props = $props();

	let modal: Modal | undefined = $state();

	export function show() {
		modal?.show();
	}

	export function hide() {
		modal?.close();
	}
</script>

<Modal bind:this={modal} width="small" type="warning" title="Resolve conflicts to preview">
	<p class="text-base-body-13 text-light">
		The file <span class="text-bold">{fileName}</span> has unresolved merge conflicts that need to be
		addressed before it can be previewed.
	</p>

	{#snippet controls()}
		<Button kind="outline" onclick={onCancel}>Cancel</Button>
		<Button style="pop" onclick={onConfirm}>Resolve Conflicts</Button>
	{/snippet}
</Modal>
