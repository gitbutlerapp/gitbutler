<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { CLAUDE_CODE_SERVICE } from '$lib/codegen/claude';
	import { extractLastMessage, usageStats, lastInteractionTime } from '$lib/codegen/messages';
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { formatNumber } from '$lib/utils/number';
	import { truncate } from '$lib/utils/string';
	import { inject } from '@gitbutler/core/context';
	import { Icon, TimeAgo, Tooltip } from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { slide, fade } from 'svelte/transition';
	import type { ClaudeStatus } from '$lib/codegen/types';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

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

	let active = $state(false);

	function getCurrentIconName(): keyof typeof iconsJson {
		if (status === 'running') {
			return 'spinner';
		}
		return 'ai';
	}
</script>

<button
	type="button"
	class="codegen-row"
	class:selected
	class:active
	onclick={() => {
		laneState.selection.set({ branchName, codegen: true, previewOpen: true });
	}}
	use:focusable={{
		onAction: () => {
			laneState.selection.set({ branchName, codegen: true, previewOpen: true });
		},
		onActive: (value) => (active = value),
		focusable: true
	}}
>
	{#if selected}
		<div
			class="indicator"
			class:selected
			class:active
			in:slide={{ axis: 'x', duration: 150 }}
		></div>
	{/if}

	<ReduxResult {projectId} result={messages.result}>
		{#snippet children(messages)}
			{@const lastMessage = extractLastMessage(messages)}
			{@const lastSummary = lastMessage ? truncate(lastMessage, 360, 8) : undefined}
			{@const usage = usageStats(messages)}
			{@const lastTime = lastInteractionTime(messages)}

			<div class="codegen-row__header">
				<div class="codegen-row__header-icon">
					<Icon name={getCurrentIconName()} size={14} />
				</div>
				<h3 class="text-13 text-semibold truncate">{lastSummary}</h3>
			</div>

			{#if usage.tokens || usage.cost}
				<div class="codegen-row__metadata text-12" in:fade={{ duration: 150 }}>
					<Tooltip text="Total tokens used and cost">
						<p>{formatNumber(usage.tokens)}</p>
						<div class="metadata-divider">|</div>
						<p>${formatNumber(usage.cost, 2)}</p>
					</Tooltip>

					{#if lastTime}
						<div class="metadata-divider">•</div>
						<p class="last-interaction-time">
							<TimeAgo date={lastTime} addSuffix />
						</p>
					{/if}
				</div>
			{/if}
		{/snippet}
	</ReduxResult>
</button>

<style lang="postcss">
	.codegen-row {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		padding: 12px;
		/* padding-left: 14px; */
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
		background: var(--clr-bg-1);
		transition: background-color var(--transition-fast);

		&:hover {
			background-color: var(--clr-bg-1-muted);
		}

		/* Selected but NOT in focus */
		&:focus-within,
		&:not(:focus-within).selected {
			background-color: var(--clr-selected-not-in-focus-bg);
		}

		/* Selected in focus */
		&.active.selected {
			background-color: var(--clr-selected-in-focus-bg);
		}
	}

	.codegen-row__header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.codegen-row__header-icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border-radius: 20px;
		background: linear-gradient(180deg, #314579 0%, #b069ce 77.5%);
		color: #fff;
	}

	.codegen-row__metadata {
		display: flex;
		gap: 6px;
		color: var(--clr-text-2);
	}

	.metadata-divider {
		color: var(--clr-text-3);
	}

	.indicator {
		position: absolute;
		top: 50%;
		left: 0;
		width: 4px;
		height: calc(100% - 28px);
		transform: translateY(-50%);
		border-radius: 0 var(--radius-ml) var(--radius-ml) 0;
		background-color: var(--clr-selected-not-in-focus-element);
		transition: transform var(--transition-fast);

		&.active {
			background-color: var(--clr-selected-in-focus-element);
		}
	}
</style>
