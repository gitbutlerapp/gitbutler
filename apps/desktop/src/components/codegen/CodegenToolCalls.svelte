<script lang="ts">
	import CodegenToolCall from '$components/codegen/CodegenToolCall.svelte';
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { getToolIcon } from '$lib/utils/codegenTools';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		toolCalls: ToolCall[];
	};
	const { projectId, toolCalls }: Props = $props();

	const filteredCalls = $derived(toolCalls.filter((tc) => tc.name !== 'TodoWrite'));

	// If only one tool call, always expanded
	let expanded = $derived(true);
	const toolDisplayLimit = 2;

	const toolsToDisplay = $derived.by(() => {
		const loadingTools = filteredCalls.filter((tc) => toolCallLoading(tc));
		const loadedTools = filteredCalls.filter((tc) => !toolCallLoading(tc));
		return [...loadingTools, ...loadedTools].slice(0, toolDisplayLimit);
	});

	function toggleExpanded() {
		expanded = !expanded;
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
					class="tool-calls-header text-13"
					onclick={toggleExpanded}
					class:expanded
				>
					<div class="tool-calls-header__arrow">
						<Icon name="chevron-right" />
					</div>
					<span class="text-bold text-12">{filteredCalls.length} tool calls</span>
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
								<p class="truncate">{toolCall.name}</p>
							</div>
						{/each}

						{#if filteredCalls.length > toolDisplayLimit}
							<span class="separator">•</span>
							<p>+<span class="tool-calls-amount"></span> more</p>
						{/if}
					{/if}
				</button>

				<!-- Content -->
				{#if expanded}
					<div class="tool-calls-expanded">
						{#each filteredCalls as toolCall}
							<CodegenToolCall {projectId} fullWidth {toolCall} />
						{/each}
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
		padding: 0 0 12px;
	}

	.tool-calls-container {
		width: fit-content;
		width: 100%;
		max-width: 100%;
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

	.tool-calls-header {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 8px 12px 12px 0;
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
		color: var(--clr-text-3);
		transition:
			background-color var(--transition-fast),
			transform var(--transition-medium);
	}

	.expanded .tool-calls-header__arrow {
		transform: rotate(90deg);
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
	}

	.separator {
		color: var(--clr-text-3);
	}
</style>
