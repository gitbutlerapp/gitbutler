<script lang="ts">
	import ScrollableContainer from '$lib/scroll/ScrollableContainer.svelte';
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { type Snippet } from 'svelte';

	interface Props {
		paddings?: {
			top?: number;
			right?: number;
			bottom?: number;
			left?: number;
		};
		children: Snippet;
	}

	const { paddings, children }: Props = $props();

	function getPaddingStyle() {
		const { top = 48, right = 32, bottom = 48, left = 32 } = paddings || {};

		return `
			padding-top: ${pxToRem(top)};
			padding-right: ${pxToRem(right)};
			padding-bottom: ${pxToRem(bottom)};
			padding-left: ${pxToRem(left)};
		`;
	}
</script>

<ScrollableContainer wide>
	<div class="wrapper">
		<div class="content" style={getPaddingStyle()}>
			{#if children}
				{@render children()}
			{/if}
		</div>
	</div>
</ScrollableContainer>

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;

		flex-grow: 1;

		align-items: center;
		justify-content: center;

		background-color: var(--clr-bg-1);
	}

	.content {
		width: 100%;
		max-width: 560px;
	}
</style>
