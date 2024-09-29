<script lang="ts">
	import { MessageRole, type UserPrompt } from '$lib/ai/types';
	import DialogBubble from '$lib/components/AIPromptEdit/DialogBubble.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import { createEventDispatcher } from 'svelte';

	interface Props {
		displayMode?: 'readOnly' | 'writable';
		prompt: UserPrompt;
	}

	let { displayMode = 'writable', prompt = $bindable() }: Props = $props();

	let expanded = $state(false);
	let editing = $state(false);
	let promptMessages = $state(structuredClone(prompt.prompt));
	let promptName = $state(structuredClone(prompt.name));
	let initialName = promptName;

	// Ensure the prompt messages have a default user prompt
	if (promptMessages.length === 0) {
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
	}

	function removeLastExample() {
		console.log(promptMessages);
		promptMessages = promptMessages.slice(0, -2);
	}

	const dispatcher = createEventDispatcher<{ deletePrompt: { prompt: UserPrompt } }>();

	function deletePrompt() {
		dispatcher('deletePrompt', { prompt });
	}

	let errorMessages = $state([] as number[]);

	function save() {
		errorMessages = checkForEmptyMessages();

		if (errorMessages.length > 0) {
			return;
		}

		if (promptName.trim() === '') {
			promptName = initialName;
		}

		prompt.prompt = promptMessages;
		prompt.name = promptName;
		editing = false;
	}

	function cancel() {
		promptMessages = structuredClone(prompt.prompt);
		promptName = structuredClone(prompt.name);
		editing = false;
	}

	let isInEditing = $derived(displayMode === 'writable' && editing);

	function toggleExpand() {
		if (isInEditing) return;

		expanded = !expanded;
	}

	function checkForEmptyMessages() {
		let errors = [] as number[];

		promptMessages.forEach((message, index) => {
			if (message.content.trim() === '') {
				errors.push(index);
			}
		});

		return errors;
	}
</script>

<div class="card">
	<div
		tabindex="0"
		role="button"
		class="header"
		class:editing={isInEditing}
		onclick={toggleExpand}
		onkeydown={(e) => e.key === 'Enter' && toggleExpand()}
	>
		{#if !isInEditing}
			<Icon name="doc" />
			<h3 class="text-15 text-bold title">{promptName}</h3>
			<div class="icon">
				<Icon name={expanded ? 'chevron-up' : 'chevron-down'} />
			</div>
		{:else}
			<TextBox bind:value={promptName} wide on:click={(e) => e.stopPropagation()} />
		{/if}
	</div>

	{#if expanded}
		<div class="content" class:default-mode={prompt.id === 'default'} class:editing={isInEditing}>
			{#each promptMessages as promptMessage, index}
				<DialogBubble
					bind:promptMessage
					editing={isInEditing}
					isLast={index + 1 === promptMessages.length || promptMessages.length === 1}
					disableRemove={promptMessages.length === 1}
					on:addExample={addExample}
					on:removeLastExample={removeLastExample}
					on:input={() => {
						errorMessages = errorMessages.filter((errorIndex) => errorIndex !== index);
					}}
					isError={errorMessages.includes(index)}
				/>

				{#if index % 2 === 0}
					<hr class="sections-divider" />
				{/if}
			{/each}
		</div>

		{#if displayMode === 'writable'}
			<div class="actions">
				{#if editing}
					<Button style="ghost" outline onclick={() => cancel()}>Cancel</Button>
					<Button
						disabled={errorMessages.length > 0}
						kind="solid"
						style="pop"
						onclick={() => save()}>Save Changes</Button
					>
				{:else}
					<Button
						style="error"
						onclick={(e: MouseEvent) => {
							e.stopPropagation();
							deletePrompt();
						}}
						icon="bin-small">Delete</Button
					>
					<Button style="ghost" outline icon="edit-text" onclick={() => (editing = true)}
						>Edit prompt</Button
					>
				{/if}
			</div>
		{/if}
	{/if}
</div>

<style lang="postcss">
	.header {
		cursor: pointer;
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 16px;

		&.editing {
			cursor: default;
		}

		& .title {
			flex: 1;
		}

		&.editing {
			cursor: default;
		}

		& .icon {
			color: var(--clr-text-2);
		}
	}

	.content {
		display: flex;
		flex-direction: column;
		gap: 16px;

		padding: 16px 0;
		border-top: 1px solid var(--clr-border-3);
	}

	.sections-divider {
		user-select: none;
		border-top: 1px solid var(--clr-border-3);
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		padding: 0 16px 16px;
	}

	.default-mode {
		padding: 16px 0;
		border-top: 1px solid var(--clr-border-3);

		& .sections-divider {
			display: none;
		}
	}
</style>
