<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';
	import type { Snippet } from 'svelte';

	interface Props {
		title?: Snippet;
		caption?: Snippet;
		actions?: Snippet;
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
		actions,
		background = 'none'
	}: Props = $props();
</script>

<div class="empty-state-container">
	<div
		class="empty-state"
		style:gap="{pxToRem(gap)}rem"
		style:max-width="{pxToRem(width)}rem"
		style:margin-bottom="{pxToRem(bottomMargin)}rem"
		style:padding={`${pxToRem(topBottomPadding)}rem ${pxToRem(leftRightPadding)}rem`}
		style:background
	>
		{#if image}
			{@html image}
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

		{#if actions}
			<div class="empty-state__actions">
				{@render actions()}
			</div>
		{/if}
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
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 100%;
		border-radius: var(--radius-m);
		color: var(--clr-scale-ntrl-60);
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
		color: var(--clr-text-2);
		opacity: 0.6;
	}

	.empty-state__caption,
	.empty-state__title {
		text-align: center;
	}

	.empty-state__actions {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 100%;
		margin-top: 8px;
		gap: 8px;
	}
</style>
