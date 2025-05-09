<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		isVerticalMode: boolean;
		children: Snippet<[boolean]>;
	};

	let { isVerticalMode, children }: Props = $props();
	let isSticked = $state(true);
</script>

<div class="sticky-buttons" class:show-bg={!isVerticalMode} class:is-sticked={isSticked}>
	{#if children}
		{@render children(isSticked)}
	{/if}
</div>

<style lang="postcss">
	.sticky-buttons {
		display: flex;
		gap: 6px;
		padding-left: 12px;
		padding-right: 12px;
		padding-bottom: 14px;

		&.show-bg {
			padding-top: 12px;
			background-color: var(--clr-bg-1);
			border-top: 1px solid var(--clr-border-2);
		}
	}
</style>
