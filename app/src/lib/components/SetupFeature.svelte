<script lang="ts">
	export let disabled = false;
	export let success = false;
	export let topBorder = false;
	const SLOTS = $$props.$$slots;
</script>

<div class="setup-feature" class:success class:disabled class:top-border={topBorder}>
	<div class="setup-feature__icon">
		<slot name="icon" />
	</div>
	<div class="setup-feature__content">
		<div class="setup-feature__title text-base-14 text-bold">
			<slot name="title" />
		</div>

		<div class="setup-feature__row">
			<div class="setup-feature__body text-base-body-12">
				<slot name="body" />
			</div>
			{#if SLOTS.actions}
				<div class="setup-feature__toggle">
					<slot name="toggle" />
				</div>
			{/if}
		</div>

		{#if SLOTS.actions}
			<div class="setup-feature__actions">
				<slot name="actions" />
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.setup-feature {
		display: flex;
		gap: var(--size-16);
		padding: var(--size-20);
	}
	.disabled.setup-feature {
		background: var(--clr-bg-2);
		opacity: 0.5;
	}
	.success.setup-feature {
		background: var(--clr-theme-pop-container, #f3fcfb);
	}

	.setup-feature__content {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		gap: var(--size-10);
	}

	.setup-feature__title {
		gap: var(--size-6);
		line-height: 120%;
	}

	.setup-feature__body {
		display: flex;
		align-items: center;
		width: 100%;
		gap: var(--size-10);
	}

	.disabled .setup-feature__icon {
		opacity: 0.5;
	}

	.setup-feature__actions {
		&:empty {
			display: none;
		}
	}

	.setup-feature__row {
		display: flex;
		gap: var(--size-10);
		align-items: center;
	}

	.setup-feature__toggle {
		line-height: 100%;
	}

	.top-border {
		border-top: 1px solid var(--clr-border-2);
	}
</style>
