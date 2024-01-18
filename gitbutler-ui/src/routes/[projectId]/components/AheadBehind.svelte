<script lang="ts">
	import Tooltip from '$lib/components/Tooltip.svelte';

	export let ahead: number | undefined;
	export let behind: number | undefined;

	$: behindMessage = `${behind} commit${behind != 1 ? 's' : ''} behind`;
	$: aheadMessage = `${ahead} commit${ahead != 1 ? 's' : ''} ahead`;
</script>

{#if ahead !== undefined && behind !== undefined}
	<Tooltip label={`This branch is ${behindMessage} and ${aheadMessage}`}>
		<div class="ahead-behind text-base-9 text-bold">
			<div class="behind" class:neutral={behind == 0}>{behind == 0 ? '0' : '-' + behind}</div>
			<div class="ahead" class:neutral={ahead == 0}>{ahead == 0 ? '0' : '+' + ahead}</div>
		</div>
	</Tooltip>
{/if}

<style lang="postcss">
	.ahead-behind {
		display: flex;
		overflow: hidden;
		flex-shrink: 0;
		line-height: 120%;
		border-radius: var(--radius-s);
		color: var(--clr-theme-scale-ntrl-40);
		background: color-mix(in srgb, var(--clr-theme-scale-ntrl-60) 30%, transparent);
	}
	.ahead,
	.behind {
		padding: var(--space-2);
		min-width: var(--space-12);
	}
	.behind {
		border-right: 1px solid var(--clr-theme-container-outline-light);
	}
	.neutral {
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>
