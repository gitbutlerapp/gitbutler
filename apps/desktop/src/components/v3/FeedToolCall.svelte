<script lang="ts">
	import Button from '@gitbutler/ui/Button.svelte';
	import type { ToolCall } from '$lib/feed/feed';

	type Props = {
		toolCall: ToolCall;
	};

	const { toolCall }: Props = $props();

	let isExpanded = $state(false);
	const parsedArgs = $derived(JSON.parse(toolCall.parameters));
	const parsedResult = $derived(JSON.parse(toolCall.result));
</script>

{#snippet code(value: unknown)}
	<div class="code-block">
		<pre class="text-11">{JSON.stringify(value, null, 2)}</pre>
	</div>
{/snippet}

<div class="tool-call-wrapper">
	<div class="tool-call">
		<p class="text-11">{toolCall.name}</p>
		<Button
			icon={isExpanded ? 'chevron-up-small' : 'chevron-down-small'}
			kind="ghost"
			onclick={() => {
				isExpanded = !isExpanded;
			}}
		/>
	</div>

	<div class="tool-call-info">
		{#if isExpanded}
			{@render code(parsedArgs)}
			{@render code(parsedResult)}
		{/if}
	</div>
</div>

<style lang="postcss">
	.tool-call-wrapper {
		display: flex;
		flex-direction: column;
		width: 100%;
	}

	.tool-call {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 0 4px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.code-block {
		padding: 4px;
		overflow-x: scroll;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2-muted);
	}

	.tool-call-info {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}
</style>
