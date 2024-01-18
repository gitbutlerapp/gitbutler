<script lang="ts">
	export let ahead: number | undefined;
	export let behind: number | undefined;
</script>

{#if ahead !== undefined && behind !== undefined}
	<div class="ahead-behind text-base-9 text-bold">
		<div class="behind" class:neutral={behind == 0}>{behind == 0 ? '0' : '-' + behind}</div>
		<div class="ahead" class:neutral={ahead == 0}>{ahead == 0 ? '0' : '+' + ahead}</div>
	</div>
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
	.ahead {
	}
	.neutral {
		color: var(--clr-theme-scale-ntrl-50);
	}
</style>
