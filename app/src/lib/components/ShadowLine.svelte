<script lang="ts">
	import type { CommitStatus } from '$lib/vbranches/types';

	export let line: boolean;
	export let first: boolean;
	export let short: boolean;
	export let dashed: boolean;
	export let upstreamLine: boolean;
	export let upstreamType: CommitStatus | undefined;
</script>

<div class="shadow-column">
	{#if line}
		{#if upstreamLine}
			<div class="shadow-line tip" class:upstream={upstreamType == 'upstream'}></div>
		{/if}
		<div class="shadow-line" class:dashed class:short class:first />
	{:else if upstreamLine}
		<div class="shadow-line upstream" class:short class:first />
	{/if}
	<slot />
</div>

<style lang="postcss">
	.shadow-column {
		position: relative;
	}

	.shadow-column {
		width: var(--size-16);
	}

	.shadow-line {
		position: absolute;
		width: var(--size-2);
		background-color: var(--clr-commit-shadow);
		left: 75%;
		bottom: 0;
		top: 0;
		&.short {
			top: calc(var(--avatar-top) + var(--size-4));
			&.first {
				top: calc(var(--avatar-first-top) + var(--size-4));
			}
		}
		&.dashed {
			background: repeating-linear-gradient(
				0,
				transparent,
				transparent 0.1875rem,
				var(--clr-commit-shadow) 0.1875rem,
				var(--clr-commit-shadow) 0.4375rem
			);
		}
		&.tip {
			bottom: calc(100% - 3.3rem);
		}
		&.upstream {
			background-color: var(--clr-commit-upstream);
		}
	}
</style>
