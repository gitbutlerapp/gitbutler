<script lang="ts">
	import Button from './Button.svelte';
	import Modal from './Modal.svelte';
	import TextBox from './TextBox.svelte';
	import { PromptService } from '$lib/backend/prompt';
	import { getContext } from '$lib/utils/context';

	const promptService = getContext(PromptService);
	const [prompt, error] = promptService.reactToPrompt({ timeoutMs: 30000 });

	let value = '';
	let modal: Modal;
	let loading = false;

	$: if ($prompt) {
		modal?.show();
	}

	$: if (!$prompt && !$error) {
		modal?.close();
	}

	async function submit() {
		if (!$prompt) return;
		loading = true;
		try {
			$prompt.respond(value);
		} catch (err) {
			console.error(err);
		} finally {
			loading = false;
			clear();
		}
	}

	async function cancel() {
		try {
			if ($prompt) $prompt.respond(null);
		} catch (err) {
			console.error(err);
		} finally {
			clear();
		}
	}

	function clear() {
		prompt.set(undefined);
		error.set(undefined);
		value = '';
	}
</script>

<Modal
	bind:this={modal}
	width="small"
	title="Git fetch requires input"
	on:submit={async () => await submit()}
	on:close={async () => await cancel()}
>
	<div class="message">
		{#if $error}
			{$error}
		{:else}
			<code>{$prompt?.prompt}</code>
		{/if}
	</div>
	<TextBox focus type="password" bind:value disabled={!!$error || loading} />

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
