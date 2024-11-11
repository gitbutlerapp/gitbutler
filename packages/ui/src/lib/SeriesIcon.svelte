<script lang="ts">
	import Icon from '$lib/Icon.svelte';
	import Tooltip from '$lib/Tooltip.svelte';

	interface Props {
		single: boolean;
		outlined?: boolean;
	}

	const { single, outlined }: Props = $props();
</script>

<Tooltip text={single ? 'Multiple branches' : 'Single branch'}>
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
		background-color: oklch(from var(--clr-scale-ntrl-60) l c h / 0.2);
		border-radius: var(--radius-m);

		&.outlined {
			border: 1px solid var(--clr-border-2);
			background: none;
		}
	}
</style>
