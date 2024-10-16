<script lang="ts">
	import TextArea from '$lib/shared/TextArea.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { TopicService, type Topic } from '$lib/topics/service';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

	interface Props {
		registerKeypress?: boolean;
		topic?: Topic;
	}

	const { registerKeypress = false, topic }: Props = $props();

	const topicService = getContext(TopicService);

	let modal = $state<Modal>();

	let title = $state(topic?.title || '');
	let body = $state(topic?.body || '');

	let submitProgress = $state<'inert' | 'loading' | 'complete'>('inert');

	async function submit() {
		submitProgress = 'loading';
		if (topic) {
			const updatedTopic = { ...topic, title, body };
			topicService.update(updatedTopic);
		} else {
			topicService.create(title, body);
		}
		submitProgress = 'complete';

		modal?.close();
	}

	export function open() {
		title = topic?.title || '';
		body = topic?.body || '';
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

	let detailsExpanded = $state(!!topic?.body);
</script>

<svelte:window on:keydown={handleKeyDown} />

<Modal bind:this={modal} onSubmit={submit}>
	<h2 class="text-18 text-bold">Create an topic</h2>

	<div class="input">
		<p class="text-14 label">Title</p>
		<TextBox bind:value={title} />
	</div>

	<div class="details">
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="details__header" onclick={() => (detailsExpanded = !detailsExpanded)}>
			<p class="text-13">Add details</p>

			{#if detailsExpanded}
				<Icon name="chevron-down" />
			{:else}
				<Icon name="chevron-up" />
			{/if}
		</div>

		<div class="details__expanded" class:hidden={!detailsExpanded}>
			<div class="input">
				<p class="text-14 label">Body</p>
				<TextArea bind:value={body} />
			</div>
		</div>
	</div>

	{#snippet controls()}
		<Button onclick={() => modal?.close()}>Cancel</Button>
		<Button kind="solid" style="pop" type="submit" loading={submitProgress === 'loading'}
			>{topic ? 'Update' : 'Create'}</Button
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

	.details {
		margin-top: 16px;
	}

	.details__header {
		display: flex;

		justify-content: space-between;
	}

	.details__expanded {
		margin-top: 8px;

		&.hidden {
			display: none;
		}
	}
</style>
