<script lang="ts">
	import Button from './Button.svelte';
	import Modal from './Modal.svelte';
	import TextBox from './TextBox.svelte';
	import { PromptService, type SystemPrompt } from '$lib/backend/prompt';
	import { getContextByClass } from '$lib/utils/context';

	const promptService = getContextByClass(PromptService);
	const prompt$ = promptService.prompt$;

	let modal: Modal;
	let prompt: SystemPrompt | undefined;
	let isSubmitting = false;
	let value = '';

	$: if ($prompt$) showPrompt($prompt$);

	function showPrompt(newPrompt: SystemPrompt) {
		if (newPrompt.context?.action == 'modal' || newPrompt.context?.branch_id === null) {
			prompt = newPrompt;
			modal.show();
		}
	}
</script>

<Modal bind:this={modal} width="small" title={prompt?.prompt}>
	<TextBox focus type="password" bind:value />

	<svelte:fragment slot="controls" let:close>
		<Button
			color="neutral"
			disabled={isSubmitting}
			kind="outlined"
			on:click={async () => {
				if (!prompt) return;
				try {
					await promptService.cancel(prompt.id);
				} catch (err) {
					console.error(err);
				} finally {
					prompt = undefined;
					close();
				}
			}}
		>
			Cancel
		</Button>
		<Button
			grow
			on:click={async () => {
				if (!prompt) return;
				isSubmitting = true;
				try {
					await promptService.respond({ id: prompt.id, response: value });
				} catch (err) {
					console.error(err);
				} finally {
					isSubmitting = false;
					value = '';
					close();
				}
			}}
			disabled={isSubmitting}
			loading={isSubmitting}
		>
			Submit
		</Button>
	</svelte:fragment>
</Modal>
