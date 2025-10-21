<script lang="ts">
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { getToolIcon } from '$lib/utils/codegenTools';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		toolCalls: ToolCall[];
	};
	const { toolCalls }: Props = $props();

	// If only one tool call, always expanded
	let expanded = $derived(toolCalls.length === 1);
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
	{#if toolCalls.length === 1}
		<!-- Only one tool call: show expanded directly, no container -->
		<CodegenToolCall toolCall={toolCalls[0]!} style="standalone" />
	{:else}
		<div class="tool-calls-wrapper">
			<div
				class="tool-calls-container"
				class:expanded
				style="--initial-tool-items: {toolCalls.length - toolDisplayLimit}"
			>
				<!-- Header for multiple tool calls -->
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
							<CodegenToolCall fullWidth {toolCall} />
						{/each}
					</div>
				{:else}
					<div class="tool-calls-collapsed text-13">
						{#each toolsToDisplay as toolCall, idx}
							<div
								class="tool-calls-collapsed__item"
								class:hidable={toolCalls.length > toolDisplayLimit}
							>
								{#if idx !== 0}
									<span class="separator">•</span>
								{/if}
								{#if toolCallLoading(toolCall)}
									<Icon name="spinner" />
								{:else}
									<Icon name={getToolIcon(toolCall.name)} color="var(--clr-text-3)" />
								{/if}
								<p class="truncate">{toolCall.name}</p>
							</div>
						{/each}

						{#if toolCalls.length > toolDisplayLimit}
							<span class="separator">•</span>
							<p>+<span class="tool-calls-amount"></span> more</p>
						{/if}
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
	}

	.tool-calls-container {
		width: fit-content;
		max-width: 100%;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);

		&.expanded {
			width: 100%;
		}
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
		overflow: hidden;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.tool-calls-collapsed__item {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.tool-calls-amount:after {
		content: counter(variable);
		counter-reset: variable calc(var(--initial-tool-items) + var(--hidden-items, 0));
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
</style>
