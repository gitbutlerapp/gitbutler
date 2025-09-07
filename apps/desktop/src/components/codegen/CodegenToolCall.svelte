<script lang="ts">
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { getToolIcon } from '$lib/utils/codegenTools';
	import { AsyncButton, Icon, Markdown } from '@gitbutler/ui';

	export type RequiresApproval = {
		onApproval: (id: string) => Promise<void>;
		onRejection: (id: string) => Promise<void>;
	};

	type Props = {
		style?: 'nested' | 'standalone';
		toolCall: ToolCall;
		requiresApproval?: RequiresApproval;
		fullWidth?: boolean;
	};
	const { toolCall, style, requiresApproval = undefined, fullWidth }: Props = $props();

	let expanded = $derived(!!requiresApproval);
</script>

<div class="tool-call {style}" class:full-width={fullWidth}>
	<button
		type="button"
		class="tool-call-header"
		class:expanded
		onclick={() => (expanded = !expanded)}
	>
		<div class="tool-call-header__sublevel"></div>

		<div class="tool-call-header__arrow">
			<Icon name="chevron-right" />
		</div>

		{#if toolCallLoading(toolCall)}
			<Icon name="spinner" size={14} />
			<p>{toolCall.name}</p>
		{:else}
			<Icon name={getToolIcon(toolCall.name)} size={14} color="var(--clr-text-3)" />
			<p class="text-13 text-left full-width">{toolCall.name}</p>
		{/if}

		{#if requiresApproval}
			<div class="flex gap-4">
				<AsyncButton
					kind="outline"
					size="tag"
					action={async () => await requiresApproval.onRejection(toolCall.id)}>Reject</AsyncButton
				>
				<AsyncButton
					style="pop"
					size="tag"
					action={async () => await requiresApproval.onApproval(toolCall.id)}>Approve</AsyncButton
				>
			</div>
		{/if}
	</button>

	{#if expanded}
		<div class="tool-call-content">
			<Markdown content={`\`\`\`\nRequest:\n${JSON.stringify(toolCall.input)}\n\`\`\``} />
			{#if toolCall.result}
				<Markdown
					content={`\`\`\`\nResponse:\n${toolCall.result.replaceAll('```', '\\`\\`\\`')}\n\`\`\``}
				/>
			{/if}
		</div>
	{/if}
</div>

<style lang="postcss">
	.tool-call {
		display: flex;
		flex-direction: column;

		max-width: 100%;
		overflow: hidden;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}

		&:not(.full-width) {
			max-width: fit-content;
		}

		&.full-width {
			width: 100%;
		}
	}

	.tool-call-header {
		display: flex;
		position: relative;
		align-items: center;
		padding: 10px 10px 10px 22px;
		gap: 8px;
		background-color: var(--clr-bg-2);

		&:hover {
			background-color: var(--clr-bg-2-muted);

			.tool-call-header__arrow {
				color: var(--clr-text-2);
			}
		}

		&.expanded {
			border-bottom: 1px solid var(--clr-border-3);

			.tool-call-header__arrow {
				transform: rotate(90deg);
			}
		}
	}

	.tool-call-header__sublevel {
		position: absolute;
		top: 0;
		left: 15px;
		width: 1px;
		height: 100%;
		background-color: var(--clr-border-2);
	}

	.tool-call-header__arrow {
		display: flex;
		color: var(--clr-text-3);
		transition:
			color var(--transition-fast),
			transform var(--transition-medium);
	}

	.tool-call-content {
		display: flex;
		flex-direction: column;
		max-width: 100%;
		padding: 12px;
		gap: 8px;
	}

	.tool-call-content :global(pre) {
		margin: 0;
	}

	/* STANDALONE MODE */
	.tool-call.standalone {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);

		.tool-call-header {
			padding-left: 12px;
			border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		}

		.tool-call-header__sublevel {
			display: none;
		}
	}
</style>
