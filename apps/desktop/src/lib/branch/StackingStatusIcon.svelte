<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';

	const FALLBACK_COLOR = 'var(--clr-scale-ntrl-80)';

	interface Props {
		icon: 'plus-small' | 'tick-small' | 'remote-branch-small';
		iconColor?: string;
		color?: string;
		lineTop?: boolean;
		lineBottom?: boolean;
		sequanceId: number;
		seqenceAmount: number;
	}

	const {
		icon,
		iconColor,
		color = FALLBACK_COLOR,
		lineTop = true,
		lineBottom = true,
		sequanceId,
		seqenceAmount
	}: Props = $props();
</script>

<div class="stack__status gap">
	<div
		class="stack__status--bar"
		style:--bg-color={lineTop ? color : 'var(--clr-transparent)'}
	></div>

	<div class="stack__status--icon" style:--bg-color={color} style:--icon-color={iconColor}>
		{#if seqenceAmount > 1}
			<span
				class="text-10 text-bold stack__status--sequence-label"
				class:small-sequance-label={seqenceAmount >= 10}>{sequanceId}/{seqenceAmount}</span
			>
		{:else}
			<Icon name={icon} />
		{/if}
	</div>
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
			/* width: 20px; */
			height: 22px;
			padding: 0 2px;
			border-radius: var(--radius-m);
			background-color: var(--bg-color);
			color: var(--icon-color, var(--clr-text-1));
		}

		& .stack__status--bar {
			width: 2px;
			height: 10px;
			margin: 0 20px;
			background: var(--bg-color);
		}

		& .stack__status--sequence-label {
			padding: 0 3px;
			text-align: center;
			line-height: 1;
		}

		& .small-sequance-label {
			font-size: 9px;
		}
	}
</style>
