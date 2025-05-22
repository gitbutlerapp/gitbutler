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
		padding-right: 12px;
		padding-bottom: 14px;
		padding-left: 12px;
		gap: 6px;

		&.show-bg {
			padding-top: 12px;
			border-top: 1px solid var(--clr-border-2);
			background-color: var(--clr-bg-1);
		}
	}
</style>
