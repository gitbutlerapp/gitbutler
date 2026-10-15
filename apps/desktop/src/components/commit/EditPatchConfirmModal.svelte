<script lang="ts">
	import { t } from "$lib/i18n";

	import { Modal, Button } from "@gitbutler/ui-svelte";

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
		{t("the-file")} <span class="text-bold">{fileName}</span>
		{t("has-unresolved-merge-conflicts-that-need-to-be-addressed-bef")}
	</p>

	{#snippet controls()}
		<Button kind="outline" onclick={onCancel}>Cancel</Button>
		<Button style="pop" onclick={onConfirm}>{t("resolve-conflicts")}</Button>
	{/snippet}
</Modal>
