<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';

	export let count: string | number;
	export let help = '';
</script>

<div class="badge text-base-10 text-bold" use:tooltip={help}>
	{count}
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
