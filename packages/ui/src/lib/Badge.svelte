<script lang="ts" context="module">
	export interface BadgeProps {
		label: string | number;
		help?: string;
	}
</script>

<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';
	let { label, help }: BadgeProps = $props();
</script>

<div class="badge text-10 text-bold" use:tooltip={help}>
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
		color: var(--clr-scale-ntrl-100);
		background-color: var(--clr-scale-ntrl-40);
		line-height: 90%;
	}
</style>
