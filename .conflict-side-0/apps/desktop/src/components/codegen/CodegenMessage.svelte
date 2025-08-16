<script lang="ts">
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import { toolCallLoading, type Message } from '$lib/codegen/messages';
	import { Avatar, Button, Icon, Markdown } from '@gitbutler/ui';

	type Props = {
		message: Message;
	};
	const { message }: Props = $props();

	let toolCallsExpanded = $state(false);

	const toolDisplayLimit = 3;

	const toolsToDisplay = $derived.by(() => {
		if (message.type !== 'claude') return [];

		const loadingTools = message.toolCalls.filter((tc) => toolCallLoading(tc));
		const loadedTools = message.toolCalls.filter((tc) => !toolCallLoading(tc));
		return [...loadingTools, ...loadedTools].slice(0, 3);
	});
</script>

{#if message.type === 'user'}
	<div class="message message-right">
		<div class="message-avatar">
			<Avatar size="large" srcUrl="https://avatars.githubusercontent.com/u/70?v=4" tooltip="" />
		</div>
		<div class="message-content">
			<div class="message-user-bubble">
				<Markdown content={message.message} />
			</div>
		</div>
	</div>
{:else if message.type === 'claude'}
	<div class="message message-left">
		<div class="message-avatar">
			{@render happyPC()}
		</div>
		<div class="message-content">
			<Markdown content={message.message} />
			{#if message.toolCalls.length > 0}
				{#if toolCallsExpanded}
					<div class="message-content-expanded-calls text-13">
						<div class="flex gap-10 items-center">
							<Button
								kind="ghost"
								icon="chevron-down"
								size="tag"
								onclick={() => (toolCallsExpanded = false)}
							/>
							<p>{message.toolCalls.length} tool calls</p>
						</div>
						{#each message.toolCalls as toolCall}
							<CodegenToolCall {toolCall} />
						{/each}
					</div>
				{:else}
					<div class="message-content-collapsed-calls text-13">
						<Button
							kind="ghost"
							icon="chevron-right"
							size="tag"
							onclick={() => (toolCallsExpanded = true)}
						/>
						<p>{message.toolCalls.length} tools in</p>
						<div class="message-content-collapsed-calls-entries clr-text-2">
							{#each toolsToDisplay as toolCall, idx}
								{#if toolCallLoading(toolCall)}
									<Icon name="spinner" />
								{/if}
								<div>{toolCall.name}</div>
								{#if idx !== toolsToDisplay.length - 1}
									<div>•</div>
								{/if}
							{/each}
						</div>
						{#if message.toolCalls.length > toolDisplayLimit}
							<p class="clr-text-2">And +{message.toolCalls.length - toolDisplayLimit} more</p>
						{/if}
					</div>
				{/if}
			{/if}
		</div>
	</div>
{/if}

{#snippet happyPC()}
	<svg width="30" height="32" viewBox="0 0 30 32" fill="none" xmlns="http://www.w3.org/2000/svg">
		<path
			d="M0.999023 12.3789C0.999023 9.0652 3.68532 6.37891 6.99902 6.37891H18.4527C21.7664 6.37891 24.4527 9.0652 24.4527 12.3789V15.3964C24.4527 17.1472 25.2175 18.8107 26.5464 19.9506L27.3212 20.6152C28.2072 21.3751 28.717 22.4841 28.717 23.6513V27.0011C28.717 29.2103 26.9262 31.0011 24.717 31.0011H6.99902C3.68532 31.0011 0.999023 28.3148 0.999023 25.0011V12.3789Z"
			fill="#F2F2DA"
			stroke="#C3C39F"
			stroke-width="1.2"
		/>
		<rect
			x="4.12793"
			y="9.45605"
			width="16.6801"
			height="18.4667"
			rx="4"
			fill="white"
			stroke="black"
			stroke-width="1.2"
		/>
		<path
			d="M7.54785 21.6074C11.2661 24.1184 13.293 24.1066 16.9027 21.6074"
			stroke="black"
			stroke-width="1.2"
		/>
		<rect x="8.2998" y="12.875" width="2.74194" height="6.57575" rx="1.37097" fill="black" />
		<rect x="13.9121" y="12.877" width="2.74194" height="6.57575" rx="1.37097" fill="black" />
		<path
			d="M21.1127 0C21.1127 4.92872 25.0916 8.92424 29.9998 8.92424C25.0916 8.92424 21.1127 12.9198 21.1127 17.8485C21.1127 12.9198 17.1338 8.92424 12.2256 8.92424C17.1338 8.92424 21.1127 4.92872 21.1127 0Z"
			fill="#24B4AD"
		/>
	</svg>
{/snippet}

<style lang="postcss">
	.message {
		display: flex;

		align-items: flex-end;
		width: 100%;
		padding: 8px 16px 16px 16px;
		gap: 8px;
	}

	.message-left {
	}

	.message-right {
		justify-content: flex-end;
	}

	.message-user-bubble {
		padding: 10px 14px;
		border-radius: var(--radius-l);
		border-bottom-left-radius: 0;
		background-color: var(--clr-bg-2);
	}

	.message-content {
		display: flex;
		flex-direction: column;
		max-width: calc(100% - 40px);
		gap: 16px;
		text-wrap: wrap;
	}

	.message-content-collapsed-calls {
		display: flex;

		align-items: center;
		width: fit-content;
		padding: 8px;
		padding-right: 12px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.message-content-collapsed-calls-entries {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.message-content-expanded-calls {
		display: flex;
		flex-direction: column;

		padding: 8px;
		gap: 10px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
