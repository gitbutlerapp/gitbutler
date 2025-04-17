<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';
	import type { Snippet } from 'svelte';

	interface Props {
		title?: Snippet;
		caption?: Snippet;
		image?: string;
		width?: number;
		bottomMargin?: number;
		gap?: number;
		topBottomPadding?: number;
		leftRightPadding?: number;
		background?: string;
	}

	const {
		image,
		width = 256,
		bottomMargin = 0,
		topBottomPadding = 48,
		leftRightPadding = 0,
		gap = 16,
		title,
		caption,
		background = 'none'
	}: Props = $props();
</script>

<div class="empty-state-container">
	<div
		class="empty-state"
		style:gap={pxToRem(gap)}
		style:max-width={pxToRem(width)}
		style:margin-bottom={pxToRem(bottomMargin)}
		style:padding={`${pxToRem(topBottomPadding)} ${pxToRem(leftRightPadding)}`}
		style:background
	>
		{#if image}
			<div class="empty-state__image">
				{@html image}
			</div>
		{/if}

		<div class="empty-state__content">
			{#if title}
				<h2 class="empty-state__title text-15 text-body text-semibold">
					{@render title()}
				</h2>
			{/if}
			{#if caption}
				<p class="empty-state__caption text-13 text-body">
					{@render caption()}
				</p>
			{/if}
		</div>
	</div>
</div>

<style>
	.empty-state-container {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		align-items: center;
		width: 100%;
		height: 100%;
	}

	.empty-state {
		user-select: none;
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		align-items: center;
		color: var(--clr-scale-ntrl-60);
		justify-content: center;
		width: 100%;
		border-radius: var(--radius-m);
		cursor: default; /* was defaulting to text cursor */
	}

	.empty-state__content {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.empty-state__title {
		color: var(--clr-scale-ntrl-40);
	}

	.empty-state__caption {
		color: var(--clr-scale-ntrl-50);
		opacity: 0.6;
	}

	.empty-state__caption,
	.empty-state__title {
		text-align: center;
	}

	.empty-state__image {
		width: 120px;
	}
</style>
