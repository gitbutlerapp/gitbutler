<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { currentActivity } from '$lib/codegen/messages';
	import { type RuleFilter } from '$lib/rules/rule';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { combineResults } from '$lib/state/helpers';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { formatNumber } from '$lib/utils/number';
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
		tokens: number;
		cost: number;
	};

	const { projectId, stackId, branchName, selected, status, tokens, cost }: Props = $props();

	const uiState = inject(UI_STATE);
	const laneState = uiState.lane(stackId);

	const rulesService = inject(RULES_SERVICE);
	const rulesQuery = rulesService.aiRuleForStack({ projectId, stackId });

	const claudeService = inject(CLAUDE_CODE_SERVICE);
	const messages = claudeService.messages({ projectId, stackId });
</script>

<ReduxResult {projectId} result={rulesQuery.result}>
	{#snippet children(rule)}
		{@const sessionId = (rule.filters[0] as RuleFilter & { type: 'claudeCodeSessionId' })?.subject}
		{@const sessionDetails = claudeService.sessionDetails(projectId, sessionId)}
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

			<ReduxResult {projectId} result={combineResults(sessionDetails.result, messages.result)}>
				{#snippet children([sessionDetails, messages])}
					{@const lastMessage = currentActivity(messages)}
					{@const lastSummary = lastMessage ? truncate(lastMessage, 300, 3) : undefined}
					{@const summary = sessionDetails.summary || undefined}
					{@const truncatedSummary = summary ? truncate(summary, 80, 1) : undefined}
					{@const truncatedLastSummary = lastSummary ? truncate(lastSummary, 80, 1) : undefined}
					{@const displaySummary = selected ? summary || lastSummary : lastSummary || summary}
					{@const truncatedDisplaySummary = selected
						? truncatedSummary || truncatedLastSummary
						: truncatedLastSummary || truncatedSummary}
					<Tooltip text={displaySummary !== truncatedSummary ? displaySummary : undefined}>
						<div class="description">
							{truncatedDisplaySummary}
						</div>
					</Tooltip>
				{/snippet}
			</ReduxResult>
			{#if !selected && status === 'running'}
				<Icon name="spinner" />
			{/if}

			{#if selected}
				<span class="tokens">
					{formatNumber(tokens, 0)}
				</span>
				<span class="p-right-12">
					${formatNumber(cost, 2)}
				</span>
			{/if}
		</button>
	{/snippet}
</ReduxResult>

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
