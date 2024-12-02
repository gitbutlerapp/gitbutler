<script lang="ts" module>
	import type { ComponentColor, ComponentStyleKind } from '$lib/utils/colorTypes';

	export interface Props {
		label: string | number;
		style?: ComponentColor;
		kind?: ComponentStyleKind;
	}
</script>

<script lang="ts">
	const { label, style = 'neutral', kind = 'solid' }: Props = $props();
</script>

<div class="badge {style} {kind} text-10 text-semibold">
	{label}
</div>

<style lang="postcss">
	.badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		height: var(--size-icon);
		min-width: var(--size-icon);
		border-radius: 14px;
		padding: 0 4px;
		line-height: 90%;
	}

	/* SOLID */

	.neutral.solid {
		color: var(--clr-scale-ntrl-100);
		background-color: var(--clr-scale-ntrl-40);
	}

	.success.solid {
		color: var(--clr-theme-succ-on-element);
		background-color: var(--clr-theme-succ-element);
	}

	.warning.solid {
		color: var(--clr-theme-warn-on-element);
		background-color: var(--clr-theme-warn-element);
	}

	.error.solid {
		color: var(--clr-theme-err-on-element);
		background-color: var(--clr-theme-err-element);
	}

	/* SOFT */
	.neutral.soft {
		color: var(--clr-text-1);
		background-color: var(--clr-scale-ntrl-80);
	}

	.success.soft {
		color: var(--clr-theme-succ-on-soft);
		background-color: var(--clr-theme-succ-soft);
	}

	.warning.soft {
		color: var(--clr-theme-warn-on-soft);
		background-color: var(--clr-theme-warn-soft);
	}

	.error.soft {
		color: var(--clr-theme-err-on-soft);
		background-color: var(--clr-theme-err-soft);
	}
</style>
