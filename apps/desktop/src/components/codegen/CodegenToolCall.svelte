<script lang="ts">
	import Codeblock from '$components/codegen/Codeblock.svelte';
	import ExpandableSection from '$components/codegen/ExpandableSection.svelte';
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { formatToolCall, getToolIcon } from '$lib/utils/codegenTools';

	type Props = {
		projectId: string;
		style?: 'nested' | 'standalone';
		toolCall: ToolCall;
		fullWidth?: boolean;
	};
	const { toolCall, style, fullWidth }: Props = $props();
</script>

<div class="tool-call {style}" class:full-width={fullWidth}>
	<ExpandableSection
		label={toolCall.name}
		icon={getToolIcon(toolCall.name)}
		loading={toolCallLoading(toolCall)}
	>
		{#snippet summary()}
			<span class="summary truncate">{formatToolCall(toolCall)}</span>
		{/snippet}

		{#snippet content()}
			{#if toolCall.result}
				<div class="stack-v gap-6 m-b-8">
					<Codeblock label="Tool Call Input:" content={formatToolCall(toolCall)} />
					<Codeblock content={toolCall.result.slice(0, 65536)} />
				</div>
			{/if}
		{/snippet}
	</ExpandableSection>
</div>

<style lang="postcss">
	.tool-call {
		&:not(.full-width) {
			max-width: var(--message-max-width);
		}

		&.full-width {
			width: 100%;
		}
		&.standalone {
			padding: 12px 0;
		}
	}

	.summary {
		padding: 3px 6px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
		font-size: 12px;
		font-family: var(--font-mono);
	}
</style>
