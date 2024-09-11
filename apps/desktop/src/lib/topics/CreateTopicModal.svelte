<script lang="ts">
	import TextArea from '$lib/shared/TextArea.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { TopicService } from '$lib/topics/service';
	import { getContext } from '$lib/utils/context';
	import { createKeybind } from '$lib/utils/hotkeys';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

	interface Props {
		registerKeypress?: boolean;
	}

	const { registerKeypress = false }: Props = $props();

	const topicService = getContext(TopicService);

	let modal = $state<Modal>();

	let title = $state('');
	let body = $state('');

	let submitProgress = $state<'inert' | 'loading' | 'complete'>('inert');

	async function submit() {
		submitProgress = 'loading';
		topicService.create(title, body);
		submitProgress = 'complete';

		modal?.close();
	}

	export function open() {
		title = '';
		body = '';
		submitProgress = 'inert';

		modal?.show();
	}

	let handleKeyDown = $state(() => {});

	$effect(() => {
		if (registerKeypress) {
			handleKeyDown = createKeybind({
				'$mod+k': open
			});
		} else {
			handleKeyDown = () => {};
		}
	});
</script>

<svelte:window on:keydown={handleKeyDown} />

<Modal bind:this={modal}>
	<h2 class="text-18 text-bold">Create an topic</h2>

	<div class="input">
		<p class="text-14 label">Title</p>
		<TextBox bind:value={title} />
	</div>

	<div class="input">
		<p class="text-14 label">Body</p>
		<TextArea bind:value={body} />
	</div>

	{#snippet controls()}
		<Button onclick={() => modal?.close()}>Cancel</Button>
		<Button kind="solid" style="pop" onclick={submit} loading={submitProgress === 'loading'}
			>Submit</Button
		>
	{/snippet}
</Modal>

<style lang="postcss">
	.input {
		margin-top: 8px;
	}

	.label {
		margin-bottom: 4px;
	}
</style>
