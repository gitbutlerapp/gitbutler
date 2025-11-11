<script lang="ts">
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { getToolIcon } from '$lib/utils/codegenTools';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		toolCalls: ToolCall[];
		messageId: string;
		toolCallsExpandedState?: Map<string, boolean>;
	};
	const { projectId, toolCalls, messageId, toolCallsExpandedState }: Props = $props();

	const filteredCalls = $derived(toolCalls.filter((tc) => tc.name !== 'TodoWrite'));

	const toolDisplayLimit = 2;

	const toolsToDisplay = $derived.by(() => {
		const loadingTools = filteredCalls.filter((tc) => toolCallLoading(tc));
		const loadedTools = filteredCalls.filter((tc) => !toolCallLoading(tc));
		return [...loadingTools, ...loadedTools].slice(0, toolDisplayLimit);
	});

	// Initialize from map, default to true
	let expanded = $state(toolCallsExpandedState?.get(messageId) ?? true);

	function toggleExpanded() {
		expanded = !expanded;
		// Persist to map if available
		if (toolCallsExpandedState) {
			toolCallsExpandedState.set(messageId, expanded);
		}
	}
</script>

{#if filteredCalls.length > 0}
	{#if filteredCalls.length === 1}
		<!-- Only one tool call: show expanded directly, no container -->
		<CodegenToolCall {projectId} toolCall={toolCalls[0]!} style="standalone" />
	{:else}
		<div class="tool-calls-wrapper">
			<div
				class="tool-calls-container"
				class:expanded
				style="--initial-tool-items: {toolCalls.length - toolDisplayLimit}"
			>
				<!-- Header for multiple tool calls -->
				<button
					type="button"
					class="text-13 tool-calls-header"
					onclick={toggleExpanded}
					class:expanded
				>
					<div class="flex gap-6 items-center">
						<div class="tool-calls-header__arrow">
							<Icon name="chevron-right" />
						</div>
						<span class="text-semibold">{filteredCalls.length} tool calls</span>
					</div>

					{#if !expanded}
						{#each toolsToDisplay as toolCall}
							<div
								class="tool-calls-collapsed__item"
								class:hidable={filteredCalls.length > toolDisplayLimit}
							>
								<span class="separator">•</span>

								{#if toolCallLoading(toolCall)}
									<Icon name="spinner" />
								{:else}
									<Icon name={getToolIcon(toolCall.name)} color="var(--clr-text-3)" />
								{/if}
								<p class="clr-text-2 truncate">{toolCall.name}</p>
							</div>
						{/each}

						{#if filteredCalls.length > toolDisplayLimit}
							<span class="separator">•</span>
							<p class="clr-text-2">+<span class="tool-calls-amount"></span> more</p>
						{/if}
					{/if}
				</button>

				<!-- Content -->
				{#if expanded}
					<div class="tool-calls-expanded">
						<div class="tool-calls-expanded__list">
							{#each filteredCalls as toolCall}
								<CodegenToolCall {projectId} fullWidth {toolCall} />
							{/each}
						</div>
					</div>
				{/if}
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
		/* background-color: red; */
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

	.tool-calls-header {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;

		&:hover {
			.tool-calls-header__arrow {
				color: var(--clr-text-2);
			}
		}
	}

	.tool-calls-header__arrow {
		display: flex;
		margin-left: -2px;
		color: var(--clr-text-3);
		transition:
			background-color var(--transition-fast),
			transform var(--transition-medium);
	}

	.expanded .tool-calls-header__arrow {
		transform: rotate(90deg);
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
		margin-top: 12px;
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
