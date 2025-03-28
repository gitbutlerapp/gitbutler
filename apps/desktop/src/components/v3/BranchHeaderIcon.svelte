<script lang="ts">
	import { getColorFromBranchType } from '$components/v3/lib';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import type { CommitStateType } from '$lib/branches/v3';

	interface Props {
		branchType: CommitStateType;
		lineTop?: boolean;
		lineBottom?: boolean;
	}

	const { branchType, lineTop = true, lineBottom = true }: Props = $props();

	const color = $derived(getColorFromBranchType(branchType));

	function getIcon(branchType: CommitStateType) {
		switch (branchType) {
			case 'LocalOnly':
				return 'branch-local';
			case 'LocalAndRemote':
				return 'branch-remote';
			case 'Integrated':
				return 'tick-small';
			default:
				return 'branch-local';
		}
	}
</script>

<div class="stack__status gap">
	<div
		class="stack__status--bar"
		style:--bg-color={lineTop ? color : 'var(--clr-transparent)'}
	></div>
	<div class="stack__status--icon" style:--bg-color={color}>
		<Icon name={getIcon(branchType)} />
	</div>
	<div
		class="stack__status--bar last"
		style:--bg-color={lineBottom ? color : 'var(--clr-transparent)'}
	></div>
</div>

<style>
	.stack__status {
		align-self: stretch;
		display: flex;
		flex-direction: column;
		justify-content: center;
		gap: 3px;
		--clr-transparent: transparent;

		& .stack__status--icon {
			display: flex;
			align-items: center;
			justify-content: center;
			width: 22px;
			height: 26px;
			border-radius: var(--radius-m);
			background-color: var(--bg-color);
			color: #fff;
			margin-left: 10px;
		}

		& .stack__status--bar {
			width: 2px;
			height: 8px;
			margin: 0 22px 0 20px;
			background: var(--bg-color);

			&.last {
				flex: 1;
			}
		}
	}
</style>
