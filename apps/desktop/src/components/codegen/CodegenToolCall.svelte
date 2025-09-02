<script lang="ts">
	import { toolCallLoading, type ToolCall } from '$lib/codegen/messages';
	import { AsyncButton, Icon, Markdown } from '@gitbutler/ui';

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
			<p class="text-13">{toolCall.name}</p>
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
		overflow: hidden;
		border-bottom: 1px solid var(--clr-border-2);

		&:last-child {
			border-bottom: none;
		}
	}

	.tool-call-header {
		display: flex;
		position: relative;
		align-items: center;
		padding: 8px 16px 8px 22px;
		gap: 8px;
		background-color: var(--clr-bg-2);

		&:hover {
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
</style>
