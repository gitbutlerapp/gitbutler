<script lang="ts">
	import ExpandableSection from "$components/shared/ExpandableSection.svelte";
	import { formatToolCall, getToolIcon, getToolLabel } from "$lib/codegen/codegenTools";
	import { toolCallLoading, type ToolCall } from "$lib/codegen/messages";
	import { Codeblock } from "@gitbutler/ui";
	import { untrack } from "svelte";

	type Props = {
		projectId: string;
		toolCall: ToolCall;
		fullWidth?: boolean;
		toolCallKey?: string;
		toolCallExpandedState?: {
			groups: Map<string, boolean>;
			individual: Map<string, boolean>;
		};
		firstInGroup: boolean;
		lastInGroup: boolean;
	};
	const {
		toolCall,
		firstInGroup,
		lastInGroup,
		fullWidth,
		toolCallKey,
		toolCallExpandedState,
	}: Props = $props();

	// Initialize expanded state from map, default to false (collapsed)
	const initialExpanded = untrack(() =>
		toolCallKey && toolCallExpandedState
			? (toolCallExpandedState.individual.get(toolCallKey) ?? false)
			: false,
	);

	function handleToggle(newExpanded: boolean) {
		// Persist to map if available
		if (toolCallKey && toolCallExpandedState) {
			toolCallExpandedState.individual.set(toolCallKey, newExpanded);
		}
	}
</script>

<div
	class="tool-call"
	class:first-in-group={firstInGroup}
	class:last-in-group={lastInGroup}
	class:full-width={fullWidth}
>
	<ExpandableSection
		label={getToolLabel(toolCall.name)}
		icon={getToolIcon(toolCall.name)}
		loading={toolCallLoading(toolCall)}
		expanded={initialExpanded}
		onToggle={handleToggle}
	>
		{#snippet summary()}
			{@const formattedCall = formatToolCall(toolCall)}
			<span class="summary truncate">{formattedCall}</span>
		{/snippet}

		{#snippet content()}
			<div class="stack-v gap-6 m-b-8">
				{#if toolCall.name !== "AskUserQuestion"}
					<Codeblock label="Tool call input:" content={formatToolCall(toolCall)} />
				{/if}
				{#if toolCall.result}
					<Codeblock content={toolCall.result.slice(0, 65536)} />
				{/if}
			</div>
		{/snippet}
	</ExpandableSection>
</div>

<style lang="postcss">
	.tool-call {
		padding-bottom: 8px;

		&:not(.full-width) {
			max-width: var(--message-max-width);
		}

		&.full-width {
			width: 100%;
		}
		&.first-in-group {
			padding-top: 12px;
		}
		&.last-in-group {
			padding-bottom: 12px;
		}
	}

	.summary {
		padding: 3px 6px;
		border-radius: var(--radius-m);
		background-color: var(--bg-2);
		color: var(--text-2);
		font-size: 12px;
		font-family: var(--font-mono);
	}
</style>
