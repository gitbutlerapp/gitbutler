<script lang="ts">
	import { t } from "$lib/i18n";

	import { AsyncButton, Button, Modal } from "@gitbutler/ui-svelte";

	interface Props {
		onSubmit: () => void;
	}

	const { onSubmit }: Props = $props();

	let modalEl = $state<ReturnType<typeof Modal>>();

	export function show() {
		modalEl?.show();
	}
	export function close() {
		modalEl?.close();
	}
</script>

<Modal bind:this={modalEl} width="small">
	<div>
		<p>{t("it-s-generally-better-to-start-resolving-conflicts-from-the-")}</p>
		<br />
		<p>{t("are-you-sure-you-want-to-resolve-conflicts-for-this-commit")}</p>
	</div>
	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<AsyncButton
			style="pop"
			action={async () => {
				await onSubmit();
				close();
			}}>Yes</AsyncButton
		>
	{/snippet}
</Modal>
