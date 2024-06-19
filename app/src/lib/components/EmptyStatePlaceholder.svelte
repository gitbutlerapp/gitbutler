<script lang="ts">
	export let image: string;
	export let width: string = '18rem';
	export let hasBottomMargin: boolean = true;
</script>

<div class="empty-state-container">
	<div
		class="empty-state"
		style:max-width={width}
		style:margin-bottom={hasBottomMargin ? '48px' : '0'}
	>
		<div class="empty-state__image">
			{@html image}
		</div>

		<div class="empty-state__content">
			{#if $$slots.title}
				<h2 class="empty-state__title text-base-body-15 text-semibold">
					<slot name="title" />
				</h2>
			{/if}
			{#if $$slots.caption}
				<p class="empty-state__caption text-base-body-13">
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
		padding: 48px 0;
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
