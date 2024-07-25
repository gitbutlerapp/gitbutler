<script lang="ts">
	export let disabled = false;
	export let success = false;
	export let topBorder = false;

	export let labelFor = '';
</script>

<label
	for={labelFor}
	class="setup-feature"
	class:success
	class:disabled
	class:top-border={topBorder}
	class:clickable={labelFor !== '' && !$$slots.actions}
>
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
			{#if $$slots.actions}
				<div class="setup-feature__toggle">
					<slot name="toggle" />
				</div>
			{/if}
		</div>

		{#if $$slots.actions}
			<div class="setup-feature__actions">
				<slot name="actions" />
			</div>
		{/if}
	</div>
</label>

<style lang="postcss">
	.setup-feature {
		display: flex;
		gap: 16px;
		padding: 20px;
	}
	.disabled.setup-feature {
		background: var(--clr-bg-2);
		opacity: 0.5;
	}
	.success.setup-feature {
		background: var(--clr-theme-pop-bg);
	}

	.setup-feature__content {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		gap: 10px;
	}

	.setup-feature__title {
		gap: 6px;
		line-height: 120%;
	}

	.setup-feature__body {
		display: flex;
		align-items: center;
		width: 100%;
		gap: 10px;
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
		gap: 10px;
		align-items: center;
	}

	.setup-feature__toggle {
		line-height: 100%;
	}

	/* MODIFIERS */

	.top-border {
		border-top: 1px solid var(--clr-border-2);
	}

	.clickable {
		cursor: pointer;
	}
</style>
