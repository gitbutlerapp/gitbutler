<script lang="ts">
	import { getForge } from '$lib/forge/interface/forge';
	import { TopicService, type Topic } from '$lib/topics/service';
	import { createKeybind } from '$lib/utils/hotkeys';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	interface Props {
		registerKeypress?: boolean;
		topic?: Topic;
	}

	const { registerKeypress = false, topic }: Props = $props();

	const forge = getForge();
	const issueService = $derived($forge?.issueService());
	const topicService = getContext(TopicService);

	let modal = $state<Modal>();
	let chooseLabelModal = $state<Modal>();

	let availables = $state<string[]>([]);
	let labels = $state<string[]>([]);

	let title = $state(topic?.title || '');
	let body = $state(topic?.body || '');

	$effect(() => {
		issueService?.listLabels().then((labels) => {
			availables = labels;
		});
	});

	let submitProgress = $state<'inert' | 'loading' | 'complete'>('inert');

	async function submit() {
		submitProgress = 'loading';
		issueService?.create(title, body, labels);
		if (topic) {
			const updatedTopic = { ...topic, title, body, hasIssue: true };
			topicService.update(updatedTopic);
		} else {
			topicService.create(title, body, true);
		}
		submitProgress = 'complete';

		modal?.close();
	}

	export function open() {
		title = topic?.title || '';
		body = topic?.body || '';
		labels = [];
		submitProgress = 'inert';

		modal?.show();
	}

	let handleKeyDown = $state(() => {});

	$effect(() => {
		if (registerKeypress && issueService) {
			handleKeyDown = createKeybind({
				'$mod+i': open
			});
		} else {
			handleKeyDown = () => {};
		}
	});
</script>

<svelte:window on:keydown={handleKeyDown} />

{#if issueService}
	<Modal bind:this={modal} onSubmit={submit}>
		<h2 class="text-18 text-bold">Create an issue</h2>

		<div class="input">
			<p class="text-14 label">Title</p>
			<Textbox bind:value={title} />
		</div>

		<div class="input">
			<p class="text-14 label">Body</p>
			<Textarea minRows={4} bind:value={body} />
		</div>

		<div class="labels">
			{#each labels as label}
				<Button onclick={() => (labels = labels.filter((l) => l !== label))} size="tag"
					>{label}</Button
				>
			{/each}

			<Modal bind:this={chooseLabelModal} width="small">
				<div class="availables">
					{#each availables.filter((label) => !labels.includes(label)) as label}
						<Button
							onclick={() => {
								labels.push(label);
								chooseLabelModal?.close();
							}}
							size="tag">{label}</Button
						>
					{/each}
				</div>
			</Modal>
			<Button icon="plus-small" size="tag" onclick={() => chooseLabelModal?.show()}
				>Add Label</Button
			>
		</div>

		{#snippet controls()}
			<Button onclick={() => modal?.close()}>Cancel</Button>
			<Button kind="solid" style="pop" type="submit" loading={submitProgress === 'loading'}
				>Submit</Button
			>
		{/snippet}
	</Modal>
{/if}

<style lang="postcss">
	.input {
		margin-top: 8px;
	}

	.label {
		margin-bottom: 4px;
	}

	.labels {
		margin-top: 8px;

		display: flex;
		flex-wrap: wrap;

		gap: 8px;
	}

	.availables {
		display: flex;
		flex-wrap: wrap;

		gap: 8px;
	}
</style>
