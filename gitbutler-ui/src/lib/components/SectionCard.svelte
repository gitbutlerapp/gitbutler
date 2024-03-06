<script lang="ts">
	export let orientation: 'row' | 'column' = 'column';
	export let hasTopRadius = true;
	export let hasBottomRadius = true;
	export let hasBottomLine = true;

	const SLOTS = $$props.$$slots;
</script>

<section
	class="section-card"
	style:flex-direction={orientation}
	class:has-top-radius={hasTopRadius}
	class:has-bottom-radius={hasBottomRadius}
	class:has-bottom-line={hasBottomLine}
>
	{#if SLOTS.iconSide}
		<div class="section-card__icon-side">
			<slot name="iconSide" />
		</div>
	{/if}

	{#if SLOTS.title || SLOTS.body}
		<div class="section-card__content">
			{#if SLOTS.title}
				<h3 class="text-base-15 text-bold section-card__title">
					<slot name="title" />
				</h3>
			{/if}
			{#if SLOTS.body}
				<p class="text-base-body-12 section-card__text">
					<slot name="body" />
				</p>
			{/if}
		</div>
	{/if}
	<slot />
</section>

<style lang="post-css">
	.section-card {
		display: flex;
		gap: var(--space-16);
		padding: var(--space-16);
		border-left: 1px solid var(--clr-theme-container-outline-light);
		border-right: 1px solid var(--clr-theme-container-outline-light);
		background-color: var(--clr-theme-container-light);
		transition:
			background-color var(--transition-fast),
			border-color var(--transition-fast);
		text-align: left;
	}

	.section-card__content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--space-8);
	}

	.section-card__title {
		color: var(--clr-theme-scale-ntrl-0);
	}

	.section-card__text {
		color: var(--clr-theme-scale-ntrl-30);
	}

	/* MODIFIERS */

	.has-top-radius {
		border-top: 1px solid var(--clr-theme-container-outline-light);
		border-top-left-radius: var(--radius-m);
		border-top-right-radius: var(--radius-m);
	}

	.has-bottom-radius {
		border-bottom-left-radius: var(--radius-m);
		border-bottom-right-radius: var(--radius-m);
	}

	.has-bottom-line {
		border-bottom: 1px solid var(--clr-theme-container-outline-light);
		user-select: text;
		cursor: text;
	}
</style>
