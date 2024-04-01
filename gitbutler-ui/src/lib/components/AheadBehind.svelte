<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';

	export let ahead: number | undefined;
	export let behind: number | undefined;

	$: behindMessage = `${behind} commit${behind != 1 ? 's' : ''} behind`;
	$: aheadMessage = `${ahead} commit${ahead != 1 ? 's' : ''} ahead`;
</script>

{#if ahead !== undefined && behind !== undefined}
	<div class="ahead-behind text-base-9 text-bold">
		<div
			use:tooltip={{ text: behindMessage, delay: 1000 }}
			class="behind"
			class:neutral={behind == 0}
		>
			{behind}
		</div>
		<div use:tooltip={{ text: aheadMessage, delay: 1000 }} class="ahead" class:neutral={ahead == 0}>
			{ahead}
		</div>
	</div>
{/if}

<style lang="postcss">
	.ahead-behind {
		display: flex;
		overflow: hidden;
		flex-shrink: 0;
		line-height: 120%;
		border-radius: var(--radius-s);
		color: var(--clr-scale-ntrl-40);
		background: color-mix(in srgb, var(--clr-scale-ntrl-60) 30%, transparent);
	}
	.ahead,
	.behind {
		padding: var(--size-2);
		min-width: var(--size-12);
	}
	.behind {
		border-right: 1px solid var(--clr-container-outline-light);
	}
	.neutral {
		color: var(--clr-scale-ntrl-50);
	}
</style>
