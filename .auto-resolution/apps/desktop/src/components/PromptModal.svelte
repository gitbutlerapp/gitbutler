<script lang="ts">
	import { PromptService } from '$lib/prompt/promptService';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	const promptService = getContext(PromptService);
	const [prompt, error] = promptService.reactToPrompt({ timeoutMs: 30000 });

	let value = $state<string>('');
	let modal = $state<ReturnType<typeof Modal>>();
	let loading = $state(false);

	$effect(() => {
		if ($prompt && modal?.imports.open === false && !loading) {
			modal?.show();
		}
	});

	async function submit() {
		if (!$prompt) return;
		loading = true;
		try {
			await modal?.close();
			await $prompt.respond(value);
		} catch (err) {
			console.error(err);
		} finally {
			loading = false;
			clear();
		}
	}

	async function cancel() {
		try {
			if ($prompt) await $prompt.respond(null);
		} catch (err) {
			console.error(err);
		} finally {
			clear();
		}
	}

	async function handleCancelButton() {
		await modal?.close();
		await cancel();
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
	title="Git needs input"
	onClickOutside={cancel}
	onSubmit={submit}
>
	<div class="message">
		{#if $error}
			{$error}
		{:else}
			<code>{$prompt?.prompt}</code>
		{/if}
	</div>
	<Textbox autofocus type="password" bind:value disabled={!!$error || loading} />

	{#snippet controls()}
		<Button kind="outline" type="reset" disabled={loading} onclick={handleCancelButton}
			>Cancel</Button
		>
		<Button style="pop" type="submit" grow disabled={!!$error || loading} {loading}>Submit</Button>
	{/snippet}
</Modal>

<style lang="postcss">
	.message {
		padding-bottom: 12px;
	}
</style>
