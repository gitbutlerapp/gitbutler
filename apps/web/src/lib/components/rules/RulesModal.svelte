<script lang="ts">
	import { parseDiffPatchToDiffString } from '$lib/chat/diffPatch';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import type { ChatMessage } from '@gitbutler/shared/chat/types';

	type Props = {
		message: ChatMessage;
	};

	const { message }: Props = $props();

	const diffString = $derived(parseDiffPatchToDiffString(message.diffPatchArray, 'after'));

	let modal = $state<Modal>();
	let ruleTitle = $state<string>();
	let ruleDescription = $state<string>();

	let ruleNegativeExample = $state<string>();
	const effectiveNegativeExample = $derived(diffString ?? ruleNegativeExample);

	let rulePositiveExample = $state<string>();
	const effectivePositiveExample = $derived(rulePositiveExample);

	const shouldShowExample = $derived(diffString !== undefined && diffString.length > 0);

	export function show() {
		modal?.show();
	}
</script>

{#snippet titleInput()}
	<div class="rules-modal__input text-input">
		<Textarea
			value={ruleTitle}
			unstyled
			placeholder="Rule title"
			fontSize={13}
			fontWeight="semibold"
			padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
			spellcheck={false}
			flex="1"
			minRows={1}
			maxRows={10}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				ruleTitle = target.value;
			}}
		/>
	</div>
{/snippet}

{#snippet descriptionInput()}
	<div class="rules-modal__input text-input">
		<Textarea
			value={ruleDescription}
			unstyled
			placeholder="Rule description"
			fontSize={13}
			padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
			spellcheck={false}
			flex="1"
			minRows={5}
			maxRows={30}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				ruleDescription = target.value;
			}}
		/>
	</div>
{/snippet}

{#snippet negativeExampleInput()}
	<div class="rules-modal__input text-input">
		<Textarea
			value={effectiveNegativeExample}
			unstyled
			fontSize={13}
			padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
			spellcheck={false}
			flex="1"
			minRows={5}
			maxRows={30}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				ruleNegativeExample = target.value;
			}}
		/>
	</div>
{/snippet}

{#snippet positiveExampleInput()}
	<div class="rules-modal__input text-input">
		<Textarea
			value={effectivePositiveExample}
			unstyled
			fontSize={13}
			padding={{ top: 0, right: 0, bottom: 0, left: 0 }}
			spellcheck={false}
			flex="1"
			minRows={5}
			maxRows={30}
			oninput={(e: Event & { currentTarget: EventTarget & HTMLTextAreaElement }) => {
				const target = e.currentTarget;
				rulePositiveExample = target.value;
			}}
		/>
	</div>
{/snippet}

<Modal bind:this={modal} title="Create a rule">
	<div class="rules-modal">
		<p>Enter the information about the rule that should be created</p>
		{@render titleInput()}
		{@render descriptionInput()}
		{#if shouldShowExample}
			<p>Examples</p>
			{@render negativeExampleInput()}
			{@render positiveExampleInput()}
		{/if}
	</div>
</Modal>

<style lang="postcss">
	.rules-modal {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.rules-modal__input {
		display: flex;
		flex-direction: column;

		width: 100%;
		gap: 8px;
	}
</style>
