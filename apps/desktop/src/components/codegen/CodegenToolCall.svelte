<script lang="ts">
	import Codeblock from '$components/codegen/Codeblock.svelte';
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { formatToolCall, getToolIcon } from '$lib/utils/codegenTools';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		style?: 'nested' | 'standalone';
		toolCall: ToolCall;
		fullWidth?: boolean;
	};
	const { toolCall, style, fullWidth }: Props = $props();

	let expanded = $state(false);
</script>

<div class="tool-call {style}" class:full-width={fullWidth}>
	<button
		type="button"
		class="tool-details text-13"
		class:expanded
		onclick={() => {
			expanded = !expanded;
		}}
	>
		<div class="tool-call-header__arrow">
			<Icon name="chevron-right" />
		</div>
		{#if toolCallLoading(toolCall)}
			<Icon name="spinner" />
		{:else}
			<Icon name={getToolIcon(toolCall.name)} color="var(--clr-text-3)" />
		{/if}

		<span class="tool-name">{toolCall.name}</span>
		<span class="summary truncate">{formatToolCall(toolCall)}</span>
	</button>

	{#if expanded && toolCall.result}
		<div class="stack-v gap-6 m-b-8">
			<Codeblock label="Tool Call Input:" content={formatToolCall(toolCall)} />
			<Codeblock content={toolCall.result.slice(0, 65536)} />
		</div>
	{/if}
</div>

<style lang="postcss">
	.tool-call {
		display: flex;
		flex-direction: column;
		max-width: 100%;
		overflow: hidden;
		gap: 12px;
		user-select: text;

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

	.tool-details {
		display: flex;
		position: relative;
		align-items: center;
		gap: 8px;

		&:hover {
			.tool-call-header__arrow {
				color: var(--clr-text-2);
			}
		}
	}

	.tool-call-header__arrow {
		display: flex;
		margin-left: -2px;
		color: var(--clr-text-3);
		transition:
			background-color var(--transition-fast),
			transform var(--transition-medium);
	}

	.expanded .tool-call-header__arrow {
		transform: rotate(90deg);
	}

	.tool-call-wrapper {
		max-height: 20lh;
		margin-bottom: 12px;
		overflow: auto;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);

		&.border {
			border: 1px solid var(--clr-border-3);
		}
	}

	.tool-call-content {
		padding: 10px 14px;
		font-family: var(--font-mono);
		white-space: pre-line;
	}

	.tool-call-content :global(pre) {
	}

	.tool-name {
		white-space: nowrap;
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
