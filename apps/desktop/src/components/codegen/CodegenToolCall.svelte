<script lang="ts">
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { AsyncButton, Button, Icon, Markdown } from '@gitbutler/ui';

	export type RequiresApproval = {
		onApproval: (id: string) => Promise<void>;
		onRejection: (id: string) => Promise<void>;
	};

	type Props = {
		toolCall: ToolCall;
		requiresApproval?: RequiresApproval;
	};
	const { toolCall, requiresApproval = undefined }: Props = $props();

	let expanded = $derived(!!requiresApproval);
</script>

<div class="tool-call">
	<div class="tool-call-header">
		<Button
			kind="ghost"
			icon={expanded ? 'chevron-down' : 'chevron-right'}
			size="tag"
			onclick={() => (expanded = !expanded)}
		/>
		{#if requiresApproval}
			<div class="flex items-center justify-between grow gap-12">
				<p>{toolCall.name} requires approval</p>
				<div class="flex gap-8">
					<AsyncButton
						kind="outline"
						action={async () => await requiresApproval.onRejection(toolCall.id)}>Reject</AsyncButton
					>
					<AsyncButton
						style="pop"
						action={async () => await requiresApproval.onApproval(toolCall.id)}>Approve</AsyncButton
					>
				</div>
			</div>
		{:else if toolCallLoading(toolCall)}
			<p>{toolCall.name}</p>
			<Icon name="spinner" />
		{:else}
			<p>{toolCall.name}</p>
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
