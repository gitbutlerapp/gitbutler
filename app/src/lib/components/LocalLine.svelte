<script lang="ts">
	import Avatar from './Avatar.svelte';
	import type { Commit } from '$lib/vbranches/types';

	export let dashed: boolean;
	export let commit: Commit | undefined;
	export let first: boolean;

	$: hasRoot = isRoot(commit);

	function isRoot(commit: Commit | undefined): boolean {
		return !!commit && (commit.parent == undefined || commit.parent?.status == 'remote');
	}
</script>

<div class="local-column">
	{#if !commit && dashed}
		<div class="local-line dashed"></div>
	{:else if commit}
		{#if first}
			<div class="local-line dashed tip" />
		{/if}
		<div class="local-line" class:has-root={hasRoot} class:short={first} />
	{/if}
	{#if commit}
		{@const author = commit.author}
		<div class="avatar" class:first>
			<Avatar {author} status={commit.status} />
		</div>
	{/if}
	{#if hasRoot}
		<div class="root" class:long-root={commit?.parent} />
	{/if}
</div>

<style lang="postcss">
	.local-column {
		position: relative;
		width: var(--size-16);
	}
	.avatar {
		position: absolute;
		top: var(--size-12);
		left: calc(-1 * var(--size-4));
		&.first {
			top: 2.625rem;
		}
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
			bottom: calc(100% - 2.625rem);
		}
		&.short {
			top: 2.625rem;
		}
	}

	.root {
		position: absolute;
		width: var(--size-10);
		top: calc(100% - var(--size-12));
		left: calc(-1 * var(--size-4));
		bottom: calc(-1 * var(--size-2));
		border-radius: 0 0 var(--radius-l) 0;
		border-color: var(--clr-commit-local);
		border-width: 0 var(--size-2) var(--size-2) 0;
		&.long-root {
			bottom: -3rem;
		}
	}
</style>
