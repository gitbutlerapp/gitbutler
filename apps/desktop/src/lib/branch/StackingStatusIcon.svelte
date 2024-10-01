<script lang="ts">
	import { getColorFromBranchType, type BranchColor } from './stackingUtils';
	import Icon from '@gitbutler/ui/Icon.svelte';

	interface Props {
		icon: 'plus-small' | 'tick-small';
		color: BranchColor;
		gap?: boolean;
		lineTop?: boolean;
	}

	const { icon, color, gap = false, lineTop = false }: Props = $props();

	// TODO: Better handle colors
	const bgColor = $derived(getColorFromBranchType(color));
</script>

<div class="stack__status gap" class:gap>
	{#if lineTop}
		<div class="stack__status--bar" style:--bg-color={bgColor}></div>
	{/if}
	<div class="stack__status--icon" style:--bg-color={bgColor}>
		<Icon name={icon} />
	</div>
	<div class="stack__status--bar" style:--bg-color={bgColor}></div>
</div>

<style>
	.stack__status {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;

		&.gap {
			gap: 0.25rem;
		}

		& .stack__status--icon {
			display: flex;
			align-items: center;
			justify-content: center;
			width: 21px;
			height: 28px;
			border-radius: 30%;
			background-color: var(--bg-color);
			color: var(--clr-text-1);
		}
		& .stack__status--bar {
			height: 10px;
			border-right: 2px solid var(--bg-color, var(--clr-border-3));
		}
	}
</style>
