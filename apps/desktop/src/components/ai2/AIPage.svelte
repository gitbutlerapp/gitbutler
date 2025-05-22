<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { Ai2Service } from '$lib/ai2/service';
	import { getContext } from '@gitbutler/shared/context';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const ai2Service = getContext(Ai2Service);

	const isOpenRouterTokenSet = ai2Service.isOpenRouterTokenSet;
	const conversations = $derived(ai2Service.conversations({ projectId }));
	const [agentCreateConversation] = ai2Service.createConversation;
	const [agentSendMessage] = ai2Service.sendMessage;

	let currentConversation = $state<string>();
	let message = $state('');

	async function createConversation({ projectId }: { projectId: string }) {
		const id = await agentCreateConversation({ projectId });
		currentConversation = id;
	}

	async function sendMessage({
		projectId,
		conversationId
	}: {
		projectId: string;
		conversationId: string;
	}) {
		await agentSendMessage({ projectId, conversationId, message });
		message = '';
	}
</script>

<div class="page">
	<div class="sidebar">
		{#if !isOpenRouterTokenSet.current.data}
			<p>
				The agent token is currently not set. Please configure it in the global experimental
				settings.
			</p>
		{/if}
		<div class="conversations">
			<p class="text-13">Conversations</p>
			<ReduxResult {projectId} result={conversations.current}>
				{#snippet children(conversations, { projectId })}
					<ul>
						{#each Object.entries(conversations) as [id, _conversation] (id)}
							<!-- svelte-ignore a11y_click_events_have_key_events -->
							<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
							<li onclick={() => (currentConversation = id)}>
								{#if currentConversation === id}
									<span>🔥</span>
								{/if}
								{id}
							</li>
						{/each}
					</ul>
					<AsyncButton
						action={async () => {
							await createConversation({ projectId });
						}}>Create new conversation</AsyncButton
					>
				{/snippet}
			</ReduxResult>
		</div>
	</div>

	<div class="main">
		<div class="conversation">
			<ReduxResult {projectId} result={conversations.current}>
				{#snippet children(conversations, { projectId })}
					{@const conversation = currentConversation
						? conversations[currentConversation]
						: undefined}
					{#if conversation}
						<ul>
							{#each conversation as message}
								<li>
									<div class="message">
										<p>Role: {message.role}</p>
										<pre>{message.content}</pre>
									</div>
								</li>
							{/each}
						</ul>
						<Textarea bind:value={message} placeholder="Could you help me with..."></Textarea>
						<AsyncButton
							action={async () => {
								await sendMessage({ projectId, conversationId: currentConversation! });
							}}>Send</AsyncButton
						>
					{/if}
				{/snippet}
			</ReduxResult>
		</div>
	</div>
</div>

<style lang="postcss">
	.page {
		display: flex;

		width: 100%;
		height: 100%;
		gap: 1rem;
	}

	.sidebar {
		width: 350px;
		height: 100%;
		padding: 1rem;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}

	.main {
		width: 100%;
		height: 100%;
		padding: 1rem;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}

	.conversation {
		height: 100%;
		overflow: auto;
	}

	.message {
		padding: 0.5rem;
		border: 1px solid var(--clr-border-1);
		border-radius: 0.5rem;
	}
</style>
