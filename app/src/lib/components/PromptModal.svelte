<script lang="ts">
	import Button from './Button.svelte';
	import Modal from './Modal.svelte';
	import TextBox from './TextBox.svelte';
	import { PromptService } from '$lib/backend/prompt';
	import { getContext } from '$lib/utils/context';
	import { TimeoutError } from 'rxjs';

	const promptService = getContext(PromptService);
	const [prompt, error] = promptService.filter({ action: 'modal', timeoutMs: 30000 });

	let value = '';
	let modal: Modal;
	let loading = false;

	$: if ($prompt) {
		modal?.show();
	}

	// TODO: Notify user we are auto closing the modal
	$: if ($error) {
		console.error($error);
		if ($error instanceof TimeoutError) {
			setTimeout(() => modal.close(), 10000);
		}
	}

	async function submit() {
		if (!$prompt) return;
		loading = true;
		try {
			await promptService.respond({ id: $prompt.id, response: value });
		} catch (err) {
			console.error(err);
		} finally {
			loading = false;
			clear();
		}
	}

	async function cancel() {
		try {
			if ($prompt) await promptService.cancel($prompt.id);
		} catch (err) {
			console.error(err);
		} finally {
			clear();
		}
	}

	function clear() {
		modal.close();
		value = '';
	}
</script>

<Modal
	bind:this={modal}
	width="small"
	title="Git fetch requires input"
	on:submit={async () => submit()}
	on:close={async () => cancel()}
>
	<div class="message">
		{#if $error}
			{$error.message}
		{:else}
			<code>{$prompt?.prompt}</code>
		{/if}
	</div>
	<TextBox focus type="password" bind:value />

	<svelte:fragment slot="controls">
		<Button style="ghost" kind="solid" type="reset" disabled={loading} on:click={cancel}>
			Cancel
		</Button>
		<Button style="pop" kind="solid" type="submit" grow disabled={!!$error || loading} {loading}
			>Submit</Button
		>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	.message {
		padding-bottom: var(--size-12);
	}
</style>
