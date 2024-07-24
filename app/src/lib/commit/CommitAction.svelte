<script lang="ts">
	import { intersectionObserver } from '$lib/utils/intersectionObserver';
	import { type Snippet } from 'svelte';

	interface Props {
		bottomBorder?: boolean;
		lines: Snippet;
		action: Snippet;
	}

	const { bottomBorder = true, lines, action }: Props = $props();

	let isNotInViewport = $state(false);
	let containerHeight = $state(0);
</script>

<div
	class="action-row sticky"
	class:not-in-viewport={isNotInViewport}
	class:sticky-z-index={isNotInViewport}
	class:bottom-border={bottomBorder}
	bind:offsetHeight={containerHeight}
	use:intersectionObserver={{
		callback: (entry) => {
			if (entry.isIntersecting) {
				console.log('entry.isIntersecting', entry.isIntersecting);
				isNotInViewport = false;
			} else {
				isNotInViewport = true;
				console.log('entry.isIntersecting', entry.isIntersecting);
			}
		},
		options: {
			root: null,
			rootMargin: `-1px 0`,
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

		background-color: var(--clr-bg-2);
		overflow: hidden;

		transition: border-top var(--transition-fast);
	}

	.action {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding-top: 10px;
		padding-right: 14px;
	}

	/* MODIFIERS */
	.bottom-border {
		border-bottom: 1px solid var(--clr-border-2);

		& .action {
			padding-bottom: 10px;
		}
	}

	.sticky {
		position: sticky;
		bottom: 0;
	}

	.sticky-z-index {
		z-index: var(--z-lifted);
	}

	.not-in-viewport {
		/* background-color: aqua; */
		box-shadow: 0 0 0 1px var(--clr-border-2);
	}
</style>
