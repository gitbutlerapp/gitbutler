<script lang="ts" context="module">
	export interface BadgeProps {
		label: string | number;
		help?: string;
		style?: 'neutral' | 'success' | 'warning' | 'error';
	}
</script>

<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';
	let { label, help, style = 'neutral' }: BadgeProps = $props();
</script>

<div class="badge {style} text-10 text-semibold" use:tooltip={help}>
	{label}
</div>

<style lang="postcss">
	.badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		height: 14px;
		min-width: 14px;
		border-radius: 14px;
		padding: 0 4px;
		line-height: 90%;
	}

	.neutral {
		color: var(--clr-scale-ntrl-100);
		background-color: var(--clr-scale-ntrl-40);
	}

	.success {
		color: var(--clr-theme-succ-on-element);
		background-color: var(--clr-theme-succ-element);
	}

	.warning {
		color: var(--clr-theme-warn-on-element);
		background-color: var(--clr-theme-warn-element);
	}

	.error {
		color: var(--clr-theme-err-on-element);
		background-color: var(--clr-theme-err-element);
	}
</style>
