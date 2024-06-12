<script lang="ts">
	import type { CommitStatus } from '$lib/vbranches/types';

	export let root: boolean = false;
	export let longRoot: boolean = false;
	export let sectionFirst: boolean = false;

	export let inType: CommitStatus | undefined;
	export let outType: CommitStatus | undefined;

	export let inDashed = false;
	export let outDashed = false;
</script>

<div class="local-column">
	{#if outType}
		<div
			class="local-line tip"
			class:first={sectionFirst}
			class:dashed={outDashed}
			class:integrated={inType === 'integrated'}
		></div>
	{/if}
	{#if inType}
		<div
			class="local-line short"
			class:dashed={inDashed}
			class:first={sectionFirst}
			class:has-root={root}
			class:integrated={inType === 'integrated'}
		></div>
	{/if}
	{#if root}
		<div class="root" class:long-root={longRoot}></div>
	{/if}
	<slot />
</div>

<style lang="postcss">
	.local-column {
		position: relative;
		width: 14px;
		/* background-color: rgba(255, 228, 196, 0.46); */
	}

	.local-line {
		position: absolute;
		width: 2px;
		background-color: var(--clr-commit-local);
		left: 4px;
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
			bottom: 8px;
		}
		&.tip {
			bottom: calc(100% - var(--avatar-top) - 4px);
			&.first {
				bottom: calc(100% - var(--avatar-first-top) - 4px);
			}
		}
		&.short {
			top: calc(var(--avatar-top) + 4px);
			&.first {
				top: calc(var(--avatar-first-top) + 4px);
			}
		}
		&.integrated {
			background-color: var(--clr-commit-shadow);
		}
	}

	.root {
		position: absolute;
		width: 10px;
		top: calc(var(--avatar-top) + 4px);
		left: -4px;
		bottom: -2px;
		border-radius: 0 0 var(--radius-l) 0;
		border-color: var(--clr-commit-local);
		border-width: 0 2px 2px 0;
		&.long-root {
			bottom: -32px;
		}
	}
</style>
