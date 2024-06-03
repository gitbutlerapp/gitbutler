<script lang="ts">
	import type { CommitStatus } from '$lib/vbranches/types';

	export let sectionFirst = false;

	export let inType: CommitStatus | undefined;
	export let outType: CommitStatus | undefined;

	export let inDashed = false;
	export let outDashed = false;
</script>

<div class="shadow-column">
	{#if outType}
		<div class="shadow-line tip" class:dashed={outDashed} class:upstream={outType == 'upstream'} />
	{/if}
	{#if inType}
		<div
			class="shadow-line short"
			class:upstream={inType == 'upstream'}
			class:first={sectionFirst}
			class:dashed={inDashed}
		/>
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
