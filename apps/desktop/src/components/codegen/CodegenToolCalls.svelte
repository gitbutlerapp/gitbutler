<script lang="ts">
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { getToolIcon } from '$lib/utils/codegenTools';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		toolCalls: ToolCall[];
	};
	const { toolCalls }: Props = $props();

	let expanded = $state(false);
	const toolDisplayLimit = 3;

	const toolsToDisplay = $derived.by(() => {
		const loadingTools = toolCalls.filter((tc) => toolCallLoading(tc));
		const loadedTools = toolCalls.filter((tc) => !toolCallLoading(tc));
		return [...loadingTools, ...loadedTools].slice(0, toolDisplayLimit);
	});

	function toggleExpanded() {
		expanded = !expanded;
	}
</script>

{#if toolCalls.length > 0}
	<div class="tool-calls-container" class:expanded>
		<!-- Header -->
		<button type="button" class="tool-calls-header" onclick={toggleExpanded}>
			<div class="tool-calls-header__arrow" class:expanded>
				<Icon name="chevron-right" />
			</div>
			<span class="text-13 text-semibold">{toolCalls.length} tool calls</span>
		</button>

		<!-- Content -->
		{#if expanded}
			<div class="tool-calls-expanded">
				{#each toolCalls as toolCall}
					<CodegenToolCall {toolCall} />
				{/each}
			</div>
		{:else}
			<div class="tool-calls-collapsed text-13">
				{#each toolsToDisplay as toolCall, idx}
					{#if toolCallLoading(toolCall)}
						<Icon name="spinner" />
					{/if}
					<Icon name={getToolIcon(toolCall.name)} color="var(--clr-text-3)" />
					<span>{toolCall.name}</span>
					{#if idx !== toolsToDisplay.length - 1}
						<span class="separator">•</span>
					{/if}
				{/each}

				{#if toolCalls.length > toolDisplayLimit}
					<span class="separator">•</span>
					<span>+{toolCalls.length - toolDisplayLimit} more</span>
				{/if}
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.tool-calls-container {
		width: fit-content;
		max-width: var(--message-max-width);
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);

		&.expanded {
			width: 100%;
		}
	}

	.tool-calls-header {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 8px 12px 8px 8px;
		gap: 8px;
		border: none;
		cursor: pointer;
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);

			.tool-calls-header__arrow {
				color: var(--clr-text-2);
			}
		}
	}

	.tool-calls-header__arrow {
		display: flex;
		color: var(--clr-text-3);
		transition:
			background-color var(--transition-fast),
			transform var(--transition-medium);

		&.expanded {
			transform: rotate(90deg);
		}
	}

	.tool-calls-collapsed {
		display: flex;
		align-items: center;
		padding: 8px 10px;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.tool-calls-expanded {
		display: flex;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		border-top: 1px solid var(--clr-border-2);
	}

	.separator {
		color: var(--clr-text-3);
	}

	.more-indicator {
		color: var(--clr-text-2);
	}
</style>
