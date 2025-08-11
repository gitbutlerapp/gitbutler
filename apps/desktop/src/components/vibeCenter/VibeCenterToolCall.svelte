<script lang="ts">
	import { toolCallLoading, type ToolCall } from '$lib/vibeCenter/transcript';
	import { Button, Icon, Markdown } from '@gitbutler/ui';

	type Props = {
		toolCall: ToolCall;
	};
	const { toolCall }: Props = $props();

	let expanded = $state(false);
</script>

<div class="tool-call">
	<div class="tool-call-header">
		<Button
			kind="ghost"
			icon={expanded ? 'chevron-down' : 'chevron-right'}
			size="tag"
			onclick={() => (expanded = !expanded)}
		/>
		<p>{toolCall.name}</p>
		{#if toolCallLoading(toolCall)}
			<Icon name="spinner" />
		{/if}
	</div>
	{#if expanded}
		<div class="tool-call-content">
			<p class="text-14 text-semibold">Request</p>
			<div class="tool-call-markdown">
				<Markdown content={`\`\`\`\n${JSON.stringify(toolCall.input)}\n\`\`\``} />
			</div>
			{#if toolCall.result}
				<p class="text-14 text-semibold">Response</p>
				<div class="tool-call-markdown">
					<Markdown content={`\`\`\`\n${toolCall.result.replaceAll('```', '\\`\\`\\`')}\n\`\`\``} />
				</div>
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.tool-call {
		display: flex;
		flex-direction: column;
		padding: 8px;

		gap: 12px;

		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.tool-call-header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.tool-call-markdown {
		max-width: 100%;
		max-height: 160px;

		overflow: auto;
	}
</style>
