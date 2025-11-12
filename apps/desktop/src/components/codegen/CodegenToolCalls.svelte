<script lang="ts">
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import ExpandableSection from '$components/codegen/ExpandableSection.svelte';
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { getToolIcon } from '$lib/utils/codegenTools';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		toolCalls: ToolCall[];
		messageId: string;
		toolCallExpandedState?: {
			groups: Map<string, boolean>;
			individual: Map<string, boolean>;
		};
	};
	const { projectId, toolCalls, messageId, toolCallExpandedState }: Props = $props();

	const filteredCalls = $derived(toolCalls.filter((tc) => tc.name !== 'TodoWrite'));

	const toolDisplayLimit = 2;

	const toolsToDisplay = $derived.by(() => {
		const loadingTools = filteredCalls.filter((tc) => toolCallLoading(tc));
		const loadedTools = filteredCalls.filter((tc) => !toolCallLoading(tc));
		return [...loadingTools, ...loadedTools].slice(0, toolDisplayLimit);
	});

	// Initialize from map, default to true
	let expanded = $state(toolCallExpandedState?.groups.get(messageId) ?? true);

	function handleToggle(newExpanded: boolean) {
		expanded = newExpanded;
		// Persist to map if available
		if (toolCallExpandedState) {
			toolCallExpandedState.groups.set(messageId, newExpanded);
		}
	}
</script>

{#if filteredCalls.length > 0}
	{#if filteredCalls.length === 1}
		<!-- Only one tool call: show expanded directly, no container -->
		<CodegenToolCall
			{projectId}
			toolCall={toolCalls[0]!}
			style="standalone"
			toolCallKey="{messageId}-0"
			{toolCallExpandedState}
		/>
	{:else}
		<div class="tool-calls-wrapper">
			<div
				class="tool-calls-container"
				style="--initial-tool-items: {toolCalls.length - toolDisplayLimit}"
			>
				<ExpandableSection
					root
					label="{filteredCalls.length} tool calls"
					bind:expanded
					onToggle={handleToggle}
				>
					{#snippet summary()}
						{#each toolsToDisplay as toolCall}
							<div
								class="tool-calls-collapsed__item"
								class:hidable={filteredCalls.length > toolDisplayLimit}
							>
								{#if toolCallLoading(toolCall)}
									<Icon name="spinner" />
								{:else}
									<Icon name={getToolIcon(toolCall.name)} color="var(--clr-text-3)" />
								{/if}
								<p class="clr-text-2 truncate">{toolCall.name}</p>
							</div>
						{/each}

						{#if filteredCalls.length > toolDisplayLimit}
							<p class="clr-text-2">+<span class="tool-calls-amount"></span> more</p>
						{/if}
					{/snippet}

					{#snippet content()}
						<div class="tool-calls-expanded">
							<div class="tool-calls-expanded__list">
								{#each filteredCalls as toolCall, index}
									<CodegenToolCall
										{projectId}
										fullWidth
										{toolCall}
										toolCallKey="{messageId}-{index}"
										{toolCallExpandedState}
									/>
								{/each}
							</div>
						</div>
					{/snippet}
				</ExpandableSection>
			</div>
		</div>
	{/if}
{/if}

<style lang="postcss">
	.tool-calls-wrapper {
		container-name: assistant-message;
		container-type: inline-size;
		padding: 12px 0;
	}

	.tool-calls-container {
		width: fit-content;
		width: 100%;
		max-width: var(--message-max-width);
		overflow: hidden;
	}

	/* Hide items in collapsed mode based on container width */
	/* and calculate the number of hidden items */
	@container assistant-message (max-width: 390px) {
		.tool-calls-container {
			--hidden-items: 1;
		}
		.hidable.tool-calls-collapsed__item:nth-child(3) {
			display: none;
		}
	}

	@container assistant-message (max-width: 300px) {
		.tool-calls-container {
			--hidden-items: 2;
		}
		.hidable.tool-calls-collapsed__item:nth-child(2) {
			display: none;
		}
	}

	.tool-calls-collapsed__item {
		display: contents;
	}

	.tool-calls-amount:after {
		content: counter(variable);
		counter-reset: variable calc(var(--initial-tool-items) + var(--hidden-items, 0));
	}

	.tool-calls-expanded {
		display: flex;
		gap: 10px;
	}

	.tool-calls-expanded__list {
		display: flex;
		flex-direction: column;
		width: 100%;
		overflow: hidden;
		gap: 8px;
	}

	.separator {
		color: var(--clr-text-3);
	}
</style>
