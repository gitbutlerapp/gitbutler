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
		header?: Snippet;
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
				lineColor: string;
				first: boolean;
				commitList?: Snippet;
		  }
	);

	let { header, branchName, ...args }: Props = $props();

	const [uiState] = inject(UiState);

	const selection = $derived(
		args.type === 'stack-branch' ? uiState.stack(args.stackId).selection.get() : undefined
	);

	const selected = $derived(selection?.current?.branchName === branchName);
</script>

{#if args.type === 'stack-branch' && !args.first}
	<BranchDividerLine lineColor={args.lineColor} />
{/if}
<div
	class="branch-card"
	class:selected
	class:draft={args.type === 'draft-branch'}
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
</style>
