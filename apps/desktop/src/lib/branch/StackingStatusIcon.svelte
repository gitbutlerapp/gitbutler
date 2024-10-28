<script lang="ts">
	import { type CommitStatus } from '$lib/vbranches/types';
	import Icon from '@gitbutler/ui/Icon.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	const FALLBACK_COLOR = 'var(--clr-scale-ntrl-80)';

	interface Props {
		icon: 'plus-small' | 'tick-small' | 'remote-branch-small';
		iconColor?: string;
		color?: string;
		lineTop?: boolean;
		lineBottom?: boolean;
		sequanceId: number;
		seqenceAmount: number;
		branchType: CommitStatus;
	}

	const {
		icon,
		iconColor,
		color = FALLBACK_COLOR,
		lineTop = true,
		lineBottom = true,
		sequanceId,
		seqenceAmount,
		branchType
	}: Props = $props();

	function branchNameType() {
		console.log(branchType);
		if (branchType === 'integrated') {
			return 'Integrated branch';
		} else if (branchType === 'localAndRemote') {
			return 'Remote branch';
		} else {
			return 'Local branch';
		}
	}
</script>

<div class="stack__status gap" class:small-sequance-label={seqenceAmount >= 10}>
	<div
		class="stack__status--bar"
		style:--bg-color={lineTop ? color : 'var(--clr-transparent)'}
	></div>

	<Tooltip
		text={seqenceAmount > 1
			? `${branchNameType()} ${sequanceId} of ${seqenceAmount}`
			: branchNameType()}
	>
		<div class="stack__status--icon" style:--bg-color={color} style:--icon-color={iconColor}>
			{#if seqenceAmount > 1}
				<span class="text-9 text-bold stack__status--sequence-label"
					>{sequanceId}/{seqenceAmount}</span
				>
			{:else}
				<Icon name={icon} />
			{/if}
		</div>
	</Tooltip>
	<div
		class="stack__status--bar"
		style:--bg-color={lineBottom ? color : 'var(--clr-transparent)'}
	></div>
</div>

<style>
	.stack__status {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		gap: 3px;
		--clr-transparent: transparent;

		& .stack__status--icon {
			display: flex;
			align-items: center;
			justify-content: center;
			min-width: 22px;
			max-width: 24px;
			padding: 4px 2px;
			border-radius: var(--radius-m);
			background-color: var(--bg-color);
			color: var(--icon-color, var(--clr-text-1));
		}

		& .stack__status--bar {
			width: 2px;
			height: 9px;
			margin: var(--lines-inner-margin);
			background: var(--bg-color);
		}

		& .stack__status--sequence-label {
			padding: 2px 3px 3px;
			text-align: center;
			line-height: 1;
		}

		/* MODIFIER */

		&.small-sequance-label {
			margin-right: 6px;

			& .stack__status--icon {
				margin-left: 4px;
				max-width: none;
			}

			& .stack__status--sequence-label {
				font-size: 9px;
			}
		}
	}
</style>
