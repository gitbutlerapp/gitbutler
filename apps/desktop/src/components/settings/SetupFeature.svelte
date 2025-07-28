<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		disabled?: boolean;
		success?: boolean;
		topBorder?: boolean;
		labelFor?: string;
		actions?: Snippet;
		icon?: Snippet;
		title?: Snippet;
		body?: Snippet;
		toggle?: Snippet;
	}

	const {
		disabled = false,
		success = false,
		topBorder = false,
		labelFor = '',
		actions,
		icon,
		title,
		body,
		toggle
	}: Props = $props();
</script>

<label
	for={labelFor}
	class="setup-feature"
	class:success
	class:disabled
	class:top-border={topBorder}
	class:clickable={labelFor !== '' && !actions}
>
	<div class="setup-feature__icon">
		{@render icon?.()}
	</div>
	<div class="setup-feature__content">
		<div class="setup-feature__title text-14 text-bold">
			{@render title?.()}
		</div>

		<div class="setup-feature__row">
			<div class="setup-feature__body text-12 text-body">
				{@render body?.()}
			</div>
			{#if actions}
				<div class="setup-feature__toggle">
					{@render toggle?.()}
				</div>
			{/if}
		</div>

		{#if actions}
			<div class="setup-feature__actions">
				{@render actions?.()}
			</div>
		{/if}
	</div>
</label>

<style lang="postcss">
	.setup-feature {
		display: flex;
		padding: 20px;
		gap: 16px;
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
		flex-grow: 1;
		flex-direction: column;
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
		align-items: center;
		gap: 10px;
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
