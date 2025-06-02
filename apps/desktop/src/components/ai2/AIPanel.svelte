<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import Resizer from '$components/Resizer.svelte';
	import { Ai2Service } from '$lib/ai2/service';
	import { getContext } from '@gitbutler/shared/context';
	import { persisted } from '@gitbutler/shared/persisted';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const panelWidth = persisted<number>(18, 'ai-panel-width');

	const ai2Service = getContext(Ai2Service);

	const [setOpenRouterToken] = ai2Service.setOpenRouterToken;
	const isOpenRouterTokenSet = ai2Service.isOpenRouterTokenSet;
	const conversations = $derived(ai2Service.conversations({ projectId }));
	const [agentCreateConversation] = ai2Service.createConversation;
	const [agentSendMessage] = ai2Service.sendMessage;

	let openRouterToken = $state('');

	$effect(() => {
		if (isOpenRouterTokenSet.current.data) {
			if (openRouterToken === '' && isOpenRouterTokenSet.current.data) {
				openRouterToken = 'xxxx';
			}
		}
	});

	let lastValue: string | undefined = undefined;

	$effect(() => {
		if (lastValue !== openRouterToken) {
			setOpenRouterToken({ token: openRouterToken });
			lastValue = openRouterToken;
		}
	});

	let panel = $state<HTMLElement | null>(null);

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

<div bind:this={panel} class="panel" style="width: {$panelWidth}rem">
	<Resizer
		viewport={panel}
		direction="left"
		minWidth={16}
		maxWidth={48}
		onWidth={(width) => {
			$panelWidth = width;
		}}
	/>
	<div class="panel__content">
		<div class="token">
			<p class="text-13">OpenRouter Token</p>
			<Textbox
				value={openRouterToken}
				onchange={(value) => {
					openRouterToken = value;
				}}
			/>
		</div>
		{#if isOpenRouterTokenSet.current.data}
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

			<div class="conversation">
				<p class="text-13">Conversation</p>
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
		{:else}
			<p>For nowsies the open router token must be set</p>
		{/if}
	</div>
</div>

<style lang="postcss">
	.panel {
		position: fixed;
		top: 0px;
		right: 0px;
		height: 100vh;

		background-color: var(--clr-bg-1);

		box-shadow: 1px 0px 10px 0px rgba(0, 0, 0, 0.1);
	}
	.panel__content {
		height: 100%;
		padding: 1rem;

		overflow: auto;
	}
	.token {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.message {
		padding: 0.5rem;
		border: 1px solid var(--clr-border-1);
		border-radius: 0.5rem;
	}
</style>
