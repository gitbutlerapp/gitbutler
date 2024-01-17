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
	}
	.ahead,
	.behind {
		min-width: 12px;
		padding: 2px;
		text-align: center;
	}
	.ahead {
		color: var(--clr-theme-scale-succ-30);
		border-radius: 0 var(--radius-s) var(--radius-s) 0;
		background: var(--clr-theme-scale-succ-80);
	}
	.behind {
		color: var(--clr-theme-scale-err-30);
		border-radius: var(--radius-s) 0 0 var(--radius-s);
		background: var(--clr-theme-scale-err-80);
	}
	.neutral {
	}
</style>
