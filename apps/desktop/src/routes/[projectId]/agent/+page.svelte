<script lang="ts">
	import { AgentFactory } from '$lib/agent/agent';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import { page } from '$app/state';

	const projectId = $derived(page.params.projectId);

	const agentFactory = getContext(AgentFactory);
	const agent = $derived(projectId ? agentFactory.createAgent(projectId) : undefined);
	const messages = $derived(agent?.messages);

	let input = $state('');

	function sendMessage() {
		agent?.userInput(input);
		input = '';
	}
</script>

<div>
	<h1>Agent</h1>

	<div class="messages">
		{#each $messages || [] as message}
			{#if message.role !== 'system'}
				<div class="message">
					<p>From: {message.role}</p>
					<pre>{message.display || message.content}</pre>
				</div>
			{/if}
		{/each}
	</div>

	<Textarea bind:value={input} />
	<Button onclick={sendMessage}>Send</Button>
</div>

<style lang="postcss">
	.messages {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.message {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;

		background-color: var(--clr-bg-1);
		padding: 1rem;
		border-radius: 0.5rem;
	}
</style>
