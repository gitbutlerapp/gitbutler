<script lang="ts">
	import Icon from '@gitbutler/ui/Icon.svelte';

	const FALLBACK_COLOR = 'var(--clr-scale-ntrl-80)';

	interface Props {
		icon: 'plus-small' | 'tick-small' | 'remote-branch-small';
		iconColor?: string;
		color?: string;
		lineTop?: boolean;
	}

	const { icon, iconColor, color = FALLBACK_COLOR, lineTop = false }: Props = $props();
</script>

<div class="stack__status gap">
	{#if lineTop}
		<div class="stack__status--bar" style:--bg-color={color}></div>
	{/if}
	<div class="stack__status--icon" style:--bg-color={color} style:--icon-color={iconColor}>
		<Icon name={icon} />
	</div>
	<div class="stack__status--bar" style:--bg-color={color}></div>
</div>

<style>
	.stack__status {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		gap: 0.25rem;

		& .stack__status--icon {
			display: flex;
			align-items: center;
			justify-content: center;
			width: 21px;
			height: 28px;
			border-radius: var(--radius-m);
			background-color: var(--bg-color);
			color: var(--icon-color, var(--clr-text-1));
		}
		& .stack__status--bar {
			height: 10px;
			border-right: 2px solid var(--bg-color, var(--clr-border-3));
		}
	}
</style>
