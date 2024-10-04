<script lang="ts">
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { type Snippet } from 'svelte';

	interface Props {
		bottomBorder?: boolean;
		backgroundColor?: boolean;
		lines: Snippet;
		action: Snippet;
	}

	const { bottomBorder = true, backgroundColor = true, lines, action }: Props = $props();

	let isNotInViewport = $state(false);
</script>

<div
	class="action-row sticky"
	class:not-in-viewport={!isNotInViewport}
	class:sticky-z-index={!isNotInViewport}
	class:bottom-border={bottomBorder}
	class:background-color={backgroundColor}
	use:intersectionObserver={{
		callback: (entry) => {
			if (entry?.isIntersecting) {
				isNotInViewport = false;
			} else {
				isNotInViewport = true;
			}
		},
		options: {
			root: null,
			rootMargin: `-100% 0px 0px 0px`,
			threshold: 0
		}
	}}
>
	<div>
		{@render lines()}
	</div>
	<div class="action">
		{@render action()}
	</div>
</div>

<style lang="postcss">
	.action-row {
		position: relative;
		display: flex;

		overflow: hidden;

		transition: border-top var(--transition-fast);
	}

	.background-color {
		background-color: var(--clr-bg-2);
	}

	.action {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding-top: 10px;
		padding-right: 14px;
		padding-bottom: 10px;
	}

	/* MODIFIERS */
	.bottom-border {
		border-bottom: 1px solid var(--clr-border-2);
	}

	.sticky {
		position: sticky;
		bottom: 0;
	}

	.sticky-z-index {
		z-index: var(--z-lifted);
	}

	.not-in-viewport {
		box-shadow: 0 0 0 1px var(--clr-border-2);
	}
</style>
