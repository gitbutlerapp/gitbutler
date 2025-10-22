<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { extractLastMessage } from '$lib/codegen/messages';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { truncate } from '$lib/utils/string';
	import { inject } from '@gitbutler/core/context';
	import { Icon, Tooltip } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';
	import type { ClaudeStatus } from '$lib/codegen/types';

	type Props = {
		projectId: string;
		stackId: string;
		branchName: string;
		selected: boolean;
		status: ClaudeStatus;
	};

	const { projectId, stackId, branchName, selected, status }: Props = $props();

	const uiState = inject(UI_STATE);
	const laneState = uiState.lane(stackId);

	const claudeService = inject(CLAUDE_CODE_SERVICE);
	const messages = claudeService.messages({ projectId, stackId });
</script>

<button
	type="button"
	class="codegen text-12"
	onclick={() => {
		laneState.selection.set({ branchName, codegen: true, previewOpen: true });
	}}
>
	{#if selected}
		<div class="active" class:selected in:slide={{ axis: 'x', duration: 150 }}></div>
	{/if}

	<ReduxResult {projectId} result={messages.result}>
		{#snippet children(messages)}
			{@const lastMessage = extractLastMessage(messages)}
			{@const lastSummary = lastMessage ? truncate(lastMessage, 360, 8) : undefined}
			{@const truncatedLastMessage = lastSummary ? truncate(lastSummary, 80, 1) : undefined}
			<Tooltip text={lastSummary !== truncatedLastMessage ? lastSummary : undefined}>
				<div class="description">
					{truncatedLastMessage}
				</div>
			</Tooltip>
		{/snippet}
	</ReduxResult>
	{#if !selected && status === 'running'}
		<Icon name="spinner" />
	{/if}
</button>

<style lang="postcss">
	.codegen {
		display: flex;
		position: relative;
		align-items: center;
		width: 100%;
		height: 32px;
		padding: 0 12px;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
	}

	.description {
		flex-grow: 1;
		overflow-x: hidden;
		text-align: left;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.selected {
		position: absolute;
		top: 50%;
		left: 0;
		width: 4px;
		height: 45%;
		transform: translateY(-50%);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--clr-selected-not-in-focus-element);
		transition: transform var(--transition-fast);
	}

	.selected.active {
		background-color: var(--clr-selected-in-focus-element);
	}
</style>
