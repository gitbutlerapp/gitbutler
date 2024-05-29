<script lang="ts">
	import { MessageRole, type UserPrompt } from '$lib/ai/types';
	import Button from '$lib/components/Button.svelte';
	import ExpandableSectionCard from '$lib/components/ExpandableSectionCard.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import TextArea from '$lib/components/TextArea.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import { createEventDispatcher } from 'svelte';

	export let displayMode: 'readOnly' | 'writable' = 'writable';
	export let prompt: UserPrompt;
	export let roundedTop: boolean;
	export let roundedBottom: boolean;

	let expanded = false;
	let editing = false;
	let promptMessages = structuredClone(prompt.prompt);
	let promptName = structuredClone(prompt.name);

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
	}

	function removeLastExample() {
		promptMessages = promptMessages.slice(0, -2);
	}

	const dispatcher = createEventDispatcher<{ deletePrompt: { prompt: UserPrompt } }>();

	function deletePrompt() {
		dispatcher('deletePrompt', { prompt });
	}

	function save() {
		prompt.prompt = promptMessages;
		prompt.name = promptName;
		editing = false;
	}

	function cancel() {
		promptMessages = structuredClone(prompt.prompt);
		promptName = structuredClone(prompt.name);
		editing = false;
	}
</script>

<ExpandableSectionCard
	{roundedTop}
	{roundedBottom}
	bind:expanded
	displayActions={displayMode == 'writable'}
	disableClosing={editing}
>
	<svelte:fragment slot="header">
		<div class="prompt-name">
			{#if displayMode == 'readOnly' || !editing}
				<p>{promptName}</p>
			{:else}
				<TextBox bind:value={promptName} wide on:click={(e) => e.stopPropagation()} />
			{/if}
			<Icon name={expanded ? 'chevron-up-small' : 'chevron-down-small'} />
		</div>
	</svelte:fragment>

	<div class="cards">
		{#each promptMessages as promptMessage, index}
			<div
				class="content-card {promptMessage.role == MessageRole.User
					? 'role-user'
					: 'role-assistant'}"
				class:editing
			>
				<div class="actions">
					{#if promptMessage.role == MessageRole.User}
						<Icon name="pull-small" />
						User
					{:else}
						<Icon name="push-small" />
						Assistant
					{/if}
					{#if index + 1 == promptMessages.length && promptMessages.length > 1 && displayMode == 'writable' && editing}
						<Button icon="bin" on:click={removeLastExample} />
					{/if}
				</div>

				<div class="prompt-messages">
					{#if displayMode == 'writable' && editing}
						<TextArea rows={2} bind:value={promptMessage.content} />
					{:else}
						<pre>{promptMessage.content}</pre>
					{/if}
				</div>
			</div>
		{/each}

		{#if displayMode == 'writable' && editing}
			<!-- svelte-ignore a11y-click-events-have-key-events -->
			<div class="content-card add-new" on:click={addExample} role="button" tabindex="0">
				<div class="actions">
					<Icon name="push-small" />
					Assistant
				</div>
				<div>Add new example</div>
			</div>
		{/if}
	</div>

	<svelte:fragment slot="actions">
		<div class="edit-actions">
			{#if editing}
				<Button kind="solid" style="ghost" on:click={() => cancel()}>Cancel</Button>
				<Button kind="solid" style="pop" on:click={() => save()}>Save Changes</Button>
			{:else}
				<Button
					style="error"
					on:click={(e) => {
						e.stopPropagation();
						deletePrompt();
					}}
					icon="bin">Delete</Button
				>
				<Button kind="solid" style="ghost" icon="edit-text" on:click={() => (editing = true)}
					>Edit Prompt</Button
				>
			{/if}
		</div>
	</svelte:fragment>
</ExpandableSectionCard>

<style lang="postcss">
	.cards {
		display: grid;
		grid-template-columns: 100%;
		gap: var(--size-16);
		width: 100%;
	}

	.content-card {
		display: flex;
		flex-direction: column;

		background-color: #fafafa;
		border-radius: var(--radius-m);
		padding: var(--size-12);

		&.role-user {
			margin-left: 60px;
		}

		&.role-assistant {
			margin-right: 60px;
		}

		&.editing {
			background-color: transparent;
			gap: var(--size-8);
			padding: 0;

			&.role-user {
				& .actions {
					justify-content: flex-end;
				}
			}
		}

		&.add-new {
			background-color: var(--clr-theme-pop-bg);
			gap: var(--size-8);
		}
	}

	.actions {
		display: flex;
		align-items: center;
		gap: var(--size-8);
	}

	.edit-actions {
		margin-left: auto;
	}

	pre {
		text-wrap: wrap;
		user-select: text;
	}

	.prompt-name {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--size-16);
	}
</style>
