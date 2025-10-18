<script lang="ts">
	import { UI_STATE } from '$lib/state/uiState.svelte';
	import { formatNumber } from '$lib/utils/number';
	import { inject } from '@gitbutler/core/context';
	import { Icon } from '@gitbutler/ui';
	import { slide } from 'svelte/transition';
	import type { ClaudeStatus } from '$lib/codegen/types';

	type Props = {
		projectId: string;
		stackId: string | undefined;
		branchName: string;
		selected: boolean;
		status: ClaudeStatus;
		tokens: number;
		cost: number;
	};

	const { stackId, branchName, selected, status, tokens, cost }: Props = $props();

	const uiState = inject(UI_STATE);
</script>

{#if stackId}
	{@const laneState = uiState.lane(stackId)}
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
		<span class="description">
			Codegen session

			{#if !selected && status === 'running'}
				<Icon name="spinner" />
			{/if}
		</span>
		<span class="tokens">
			{formatNumber(tokens, 0)}
		</span>
		<span class="p-right-12">
			${formatNumber(cost, 2)}
		</span>
	</button>
{/if}

<style lang="postcss">
	.codegen {
		display: flex;
		position: relative;
		align-items: center;
		width: 100%;
		height: 32px;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
	}

	.description {
		flex-grow: 1;
		margin-left: 12px;
		text-align: left;
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
