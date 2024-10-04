<script lang="ts">
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	export let image: string;
	export let width: number = 256;
	export let bottomMargin: number = 0;
	export let topBottomPadding: number = 48;
	export let leftRightPadding: number = 0;
</script>

<div class="empty-state-container">
	<div
		class="empty-state"
		style:max-width={pxToRem(width)}
		style:margin-bottom={pxToRem(bottomMargin)}
		style:padding={`${pxToRem(topBottomPadding)} ${pxToRem(leftRightPadding)}`}
	>
		<div class="empty-state__image">
			{@html image}
		</div>

		<div class="empty-state__content">
			{#if $$slots.title}
				<h2 class="empty-state__title text-15 text-body text-semibold">
					<slot name="title" />
				</h2>
			{/if}
			{#if $$slots.caption}
				<p class="empty-state__caption text-13 text-body">
					<slot name="caption" />
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
		background: var(--clr-bg-1);
		justify-content: center;
		width: 100%;
		gap: 16px;
		border-radius: var(--radius-m);
		cursor: default; /* was defaulting to text cursor */
	}

	.empty-state__content {
		display: flex;
		flex-direction: column;
		gap: 8px;
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
