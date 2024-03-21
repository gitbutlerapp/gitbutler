<script lang="ts">
	import Button from './Button.svelte';
	import Modal from './Modal.svelte';
	import TextBox from './TextBox.svelte';
	import { PromptService } from '$lib/backend/prompt';
	import { getContextByClass } from '$lib/utils/context';
	import { TimeoutError } from 'rxjs';

	const promptService = getContextByClass(PromptService);
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
		console.log('clearing');
		modal.close();
		value = '';
	}
</script>

<Modal bind:this={modal} width="small" title="Git fetch requires input">
	<div class="message">
		{#if $error}
			{$error.message}
		{:else}
			<code>{$prompt?.prompt}</code>
		{/if}
	</div>
	<TextBox
		focus
		type="password"
		bind:value
		on:keydown={(e) => {
			if (e.detail.key == 'Enter') submit();
		}}
	/>

	<svelte:fragment slot="controls">
		<Button
			color="neutral"
			disabled={loading}
			kind="outlined"
			on:click={() => {
				cancel();
			}}
		>
			Cancel
		</Button>
		<Button grow disabled={!!$error || loading} on:click={async () => await submit()} {loading}>
			Submit
		</Button>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	.message {
		padding-bottom: var(--size-12);
	}
</style>
