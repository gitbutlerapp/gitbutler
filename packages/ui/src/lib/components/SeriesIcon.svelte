<script lang="ts">
	import Icon from '$components/Icon.svelte';
	import Tooltip from '$components/Tooltip.svelte';

	interface Props {
		single: boolean;
		origin?: boolean;
		outlined?: boolean;
	}

	const { single, origin, outlined }: Props = $props();

	function getIcon() {
		if (origin) {
			return 'home-small';
		}

		if (single) {
			return 'branch-small';
		}

		return 'link-small';
	}
</script>

<Tooltip text={single ? 'Single branch' : 'Multiple branches'}>
	<div class="stack-icon" class:outlined>
		<Icon name={getIcon()} />
	</div>
</Tooltip>

<style>
	.stack-icon {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		border-radius: var(--radius-s);
		box-shadow: inset 0 0 0 1px var(--clr-border-2);
		color: var(--clr-text-2);
	}
</style>
