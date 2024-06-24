<script lang="ts">
	import { MessageRole } from '$lib/ai/types';
	import Button from '$lib/shared/Button.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import { useAutoHeight } from '$lib/utils/useAutoHeight';
	import { marked } from 'marked';
	import { createEventDispatcher } from 'svelte';

	export let disableRemove = false;
	export let isError = false;
	export let isLast = false;
	export let autofocus = false;
	export let editing = false;
	export let promptMessage: { role: MessageRole; content: string };

	const dispatcher = createEventDispatcher<{
		removeLastExample: void;
		addExample: void;
		input: string;
	}>();
	let textareaElement: HTMLTextAreaElement | undefined;

	function focusTextareaOnMount(
		textareaElement: HTMLTextAreaElement | undefined,
		autofocus: boolean,
		editing: boolean
	) {
		if (textareaElement && autofocus && editing) {
			textareaElement.scrollIntoView({ behavior: 'smooth', block: 'center' });
			textareaElement.focus();
		}
	}

	$: focusTextareaOnMount(textareaElement, autofocus, editing);

	$: if (textareaElement) useAutoHeight(textareaElement);
</script>

<div
	class="bubble-wrap"
	class:editing
	class:bubble-wrap_user={promptMessage.role === MessageRole.User}
	class:bubble-wrap_assistant={promptMessage.role === MessageRole.Assistant}
>
	<div class="bubble">
		<div class="bubble__header text-base-13 text-bold">
			{#if promptMessage.role === MessageRole.User}
				<Icon name="profile" />
				<span>User</span>
			{:else}
				<Icon name="robot" />
				<span>Assistant</span>
			{/if}
		</div>

		{#if editing}
			<textarea
				bind:this={textareaElement}
				bind:value={promptMessage.content}
				class="textarea scrollbar text-base-body-13"
				class:is-error={isError}
				rows={1}
				on:input={(e) => {
					useAutoHeight(e.currentTarget);

					dispatcher('input', e.currentTarget.value);
				}}
				on:change={(e) => {
					useAutoHeight(e.currentTarget);
				}}
			></textarea>
		{:else}
			<div class="markdown bubble-message scrollbar text-base-body-13">
				{@html marked.parse(promptMessage.content)}
			</div>
		{/if}
	</div>

	{#if isLast && editing}
		<div class="bubble-actions">
			{#if !disableRemove}
				<Button
					icon="bin-small"
					kind="soft"
					style="error"
					on:click={() => dispatcher('removeLastExample')}
				>
					Remove example
				</Button>
			{/if}
			<Button style="ghost" outline grow on:click={() => dispatcher('addExample')}>
				Add new example
			</Button>
		</div>
	{/if}
</div>

<style lang="postcss">
	.bubble-wrap {
		display: flex;
		flex-direction: column;

		width: 100%;
		padding: 0 16px;

		&.editing {
			& .bubble__header {
				border: 1px solid var(--clr-border-2);
				border-bottom: none;
			}
		}
	}

	.bubble {
		width: 100%;
		max-width: 90%;
		/* overflow: hidden; */
	}

	.bubble-wrap_user {
		align-items: flex-end;

		& .bubble__header,
		& .bubble-message {
			background-color: var(--clr-bg-2);
		}
	}

	.bubble-wrap_assistant {
		align-items: flex-start;

		& .bubble__header,
		& .bubble-message {
			background-color: var(--clr-theme-pop-bg);
		}
	}

	.bubble__header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 12px;
		/* border: 1px solid var(--clr-border-2); */

		border-bottom: none;
		border-radius: var(--radius-l) var(--radius-l) 0 0;
	}

	.bubble-message {
		overflow-x: auto;
		color: var(--clr-text-1);
		border-top: 1px solid var(--clr-border-2);
		/* border: 1px solid var(--clr-border-2); */

		border-radius: 0 0 var(--radius-l) var(--radius-l);
		padding: 12px;
	}

	.bubble-actions {
		display: flex;
		width: 90%;
		margin-top: 12px;
		margin-bottom: 8px;
		gap: 8px;
	}

	.textarea {
		width: 100%;
		resize: none;
		background: none;
		border: none;
		outline: none;
		padding: 12px;
		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-l) var(--radius-l);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);

		&:not(.is-error):hover,
		&:not(.is-error):focus-within {
			border-color: var(--clr-border-1);
		}
	}

	/* MODIFIERS */
	.is-error {
		animation: shake 0.25s ease-in-out forwards;
	}

	@keyframes shake {
		0% {
			transform: translateX(0);
		}
		25% {
			transform: translateX(-5px);
		}
		50% {
			transform: translateX(5px);
			border: 1px solid var(--clr-theme-err-element);
		}
		75% {
			transform: translateX(-5px);
			border: 1px solid var(--clr-theme-err-element);
		}
		100% {
			transform: translateX(0);
			border: 1px solid var(--clr-theme-err-element);
		}
	}
</style>
