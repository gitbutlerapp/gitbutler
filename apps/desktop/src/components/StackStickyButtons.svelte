<script lang="ts">
	import { stickyHeader } from '@gitbutler/ui/utils/stickyHeader';
	import type { Snippet } from 'svelte';

	type Props = {
		children: Snippet<[boolean]>;
	};

	const { children }: Props = $props();
	let isSticked = $state(true);
</script>

<div
	class="sticky-buttons"
	class:is-sticked={isSticked}
	use:stickyHeader={{
		align: 'bottom',
		onStick: (sticky) => {
			isSticked = sticky;
		},
		unstyled: true
	}}
>
	{#if children}
		{@render children(isSticked)}
	{/if}
</div>

<style lang="postcss">
	.sticky-buttons {
		z-index: var(--z-lifted);
		padding: 8px 0 8px;
		margin-bottom: -9px;
		transition: padding var(--transition-medium);
		display: flex;
		gap: 6px;

		&:after {
			content: '';
			display: block;
			position: absolute;
			bottom: 0;
			left: -14px;
			height: calc(100% + 8px);
			width: calc(100% + 28px);
			z-index: -1;
			background-color: var(--clr-bg-1);
			border-top: 1px solid var(--clr-border-2);

			transform: translateY(10%);
			opacity: 0;
			transition:
				opacity var(--transition-fast),
				transform var(--transition-medium);
		}

		&.is-sticked {
			padding-bottom: 14px;
		}

		&.is-sticked:after {
			opacity: 1;
			transform: translateY(0);
		}
	}
</style>
