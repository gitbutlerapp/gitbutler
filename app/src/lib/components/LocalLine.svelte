<script lang="ts">
	import type { CommitStatus } from '$lib/vbranches/types';

	export let dashed: boolean;
	export let first: boolean;
	export let isEmpty: boolean = false;
	export let root: boolean = false;
	export let longRoot: boolean = false;
	export let nextType: CommitStatus | undefined;
</script>

<div class="local-column">
	{#if !isEmpty}
		{#if nextType}
			<div class="local-line dashed tip" />
		{/if}
		<div class="local-line" class:dashed class:first class:has-root={root} class:short={first} />
		{#if root}
			<div class="root" class:long-root={longRoot} />
		{/if}
		<slot />
	{/if}
</div>

<style lang="postcss">
	.local-column {
		position: relative;
		width: var(--size-14);
		/* background-color: rgba(255, 228, 196, 0.46); */
	}

	.local-line {
		position: absolute;
		width: var(--size-2);
		background-color: var(--clr-commit-local);
		left: var(--size-4);
		top: 0;
		bottom: 0;
		&.dashed {
			background: repeating-linear-gradient(
				0,
				transparent,
				transparent 0.1875rem,
				var(--clr-commit-local) 0.1875rem,
				var(--clr-commit-local) 0.4375rem
			);
		}
		&.has-root {
			bottom: var(--size-8);
		}
		&.tip {
			bottom: calc(100% - var(--avatar-first-top));
		}
		&.short {
			top: var(--avatar-first-top);
		}
	}

	.root {
		position: absolute;
		width: var(--size-10);
		top: calc(100% - var(--size-14));
		left: calc(-1 * var(--size-4));
		bottom: calc(-1 * var(--size-2));
		border-radius: 0 0 var(--radius-l) 0;
		border-color: var(--clr-commit-local);
		border-width: 0 var(--size-2) var(--size-2) 0;
		&.long-root {
			bottom: -2rem;
		}
	}
</style>
