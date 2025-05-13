<script lang="ts">
	import BranchDividerLine from '$components/v3/BranchDividerLine.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { Snippet } from 'svelte';

	type Props = {
		type: 'draft-branch' | 'normal-branch' | 'stack-branch';
		projectId: string;
		branchName: string;
		isCommitting?: boolean;
		expand?: boolean;
		header?: Snippet;
		first?: boolean;
		lineColor?: string;
	} & (
		| {
				type: 'draft-branch';
		  }
		| {
				type: 'normal-branch';
				commitList?: Snippet;
		  }
		| {
				type: 'stack-branch';
				stackId: string;
				commitList?: Snippet;
		  }
	);

	let { header, branchName, expand, ...args }: Props = $props();

	const [uiState] = inject(UiState);

	const stackState = $derived(
		args.type === 'stack-branch' ? uiState.stack(args.stackId) : undefined
	);
	const selection = $derived(stackState ? stackState.selection.current : undefined);
	const selected = $derived(selection?.branchName === branchName);
</script>

{#if (args.type === 'stack-branch' || (args.type === 'normal-branch' && !args.first)) && args.lineColor}
	<BranchDividerLine lineColor={args.lineColor} />
{/if}

<div
	class="branch-card"
	class:selected
	class:draft={args.type === 'draft-branch'}
	class:expand
	data-series-name={branchName}
>
	{@render header?.()}
	{#if args.type !== 'draft-branch'}
		{@render args.commitList?.()}
	{/if}
</div>

<style>
	.branch-card {
		display: flex;
		flex-direction: column;
		width: 100%;
		position: relative;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background: var(--clr-bg-1);
		&.draft {
			border-radius: var(--radius-ml) var(--radius-ml) 0 0;
		}
	}
	.expand {
		height: 100%;
	}
</style>
