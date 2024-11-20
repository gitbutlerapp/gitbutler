<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';

	interface Props {
		single: boolean;
		outlined?: boolean;
	}

	const { single, outlined }: Props = $props();
</script>

<Tooltip text={single ? 'Single branch' : 'Multiple branches'}>
	<div class="stack-icon" class:outlined>
		<Icon name={single ? 'chain-link' : 'branch-small'} />
	</div>
</Tooltip>

<style>
	.stack-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		width: 20px;
		height: 22px;
		color: var(--clr-text-2);
		background-color: oklch(from var(--clr-core-ntrl-60) l c h / 0.15);
		border-radius: var(--radius-m);

		&.outlined {
			border: 1px solid var(--clr-border-2);
			background: none;
		}
	}
</style>
