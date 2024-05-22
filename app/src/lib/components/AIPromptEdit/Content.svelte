<script lang="ts">
	import { MessageRole, type Prompt } from '$lib/ai/types';
	import Button from '$lib/components/Button.svelte';
	import Tag from '$lib/components/Tag.svelte';
	import TextArea from '$lib/components/TextArea.svelte';

	export let displayMode: 'readOnly' | 'writable' = 'writable';
	export let promptMessages: Prompt;

	// Ensure the prompt messages have a default user prompt
	if (promptMessages.length == 0) {
		promptMessages = [
			...promptMessages,
			{
				role: MessageRole.User,
				content: ''
			}
		];
	}

	function addExample() {
		promptMessages = [
			...promptMessages,
			{
				role: MessageRole.Assistant,
				content: ''
			},
			{
				role: MessageRole.User,
				content: ''
			}
		];

		console.log(promptMessages);
	}

	function removeLastExample() {
		promptMessages = promptMessages.slice(0, -2);
	}
</script>

<div class="cards">
	{#each promptMessages as promptMessage, index}
		<div class="content-card">
			<div class="actions">
				{#if promptMessage.role == MessageRole.User}
					<Tag kind="soft" style="pop" shrinkable>User</Tag>
				{:else}
					<Tag kind="soft" style="neutral" shrinkable>Assistant</Tag>
				{/if}
				{#if index + 1 == promptMessages.length && promptMessages.length > 1 && displayMode == 'writable'}
					<Button icon="bin" on:click={removeLastExample} />
				{/if}
			</div>

			{#if displayMode == 'writable'}
				<TextArea bind:value={promptMessage.content} resizeable />
			{:else}
				<pre>{promptMessage.content}</pre>
			{/if}
		</div>
	{/each}

	{#if displayMode == 'writable'}
		<div class="content-card">
			<Tag kind="soft" style="neutral" shrinkable>Assistant</Tag>
			<div>
				<Button on:click={addExample}>Add an example</Button>
			</div>
		</div>
	{/if}
</div>

<style lang="postcss">
	.cards {
		display: grid;
		grid-template-columns: 100%;
		gap: 8px;
	}

	.content-card {
		display: flex;
		flex-direction: column;
		gap: 8px;

		background-color: #fafafa;
		border: 1px solid #efefef;
		border-radius: var(--radius-s);
		padding: var(--size-8);
	}

	.actions {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	pre {
		text-wrap: wrap;
		user-select: text;
	}
</style>
