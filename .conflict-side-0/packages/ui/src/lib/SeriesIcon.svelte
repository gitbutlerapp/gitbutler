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
		color: var(--clr-text-2);
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		border-radius: var(--radius-s);
	}
</style>
