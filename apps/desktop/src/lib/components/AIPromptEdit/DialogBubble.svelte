<script lang="ts">
	import { MessageRole } from '$lib/ai/types';
	import Markdown from '$lib/components/Markdown.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	interface Props {
		role: MessageRole;
		disableRemove?: boolean;
		isError?: boolean;
		isLast?: boolean;
		editing?: boolean;
		promptMessage: string;
		onRemoveLastExample: () => void;
		onAddExample: () => void;
		onInput: (value: string) => void;
	}

	let {
		role,
		disableRemove = false,
		isError = false,
		isLast = false,
		editing = false,
		promptMessage = $bindable(),
		onRemoveLastExample,
		onAddExample,
		onInput
	}: Props = $props();
</script>

<div
	class="bubble-wrap"
	class:editing
	class:bubble-wrap_user={role === MessageRole.User}
	class:bubble-wrap_assistant={role === MessageRole.Assistant}
>
	<div class="bubble">
		<div class="bubble__header text-13 text-bold">
			{#if role === MessageRole.User}
				<Icon name="profile" />
				<span>User</span>
			{:else}
				<Icon name="robot" />
				<span>Assistant</span>
			{/if}
		</div>

		{#if editing}
			<div class="textarea" class:is-error={isError}>
				<Textarea
					unstyled
					bind:value={promptMessage}
					oninput={(e: Event) => {
						const target = e.currentTarget as HTMLTextAreaElement;
						onInput(target.value);
					}}
				></Textarea>
			</div>
		{:else}
			<div class="bubble-message scrollbar text-13 text-body">
				<Markdown content={promptMessage} />
			</div>
		{/if}
	</div>

	{#if isLast && editing}
		<div class="bubble-actions">
			{#if !disableRemove}
				<Button icon="bin-small" kind="soft" style="error" onclick={() => onRemoveLastExample()}>
					Remove example
				</Button>
			{/if}
			<Button style="ghost" outline grow onclick={() => onAddExample()}>Add new example</Button>
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
