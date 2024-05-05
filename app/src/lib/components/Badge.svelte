<script lang="ts">
	import { tooltip } from '$lib/utils/tooltip';

	export let count: string | number;
	export let help = '';
</script>

<div class="badge text-base-9 text-semibold" use:tooltip={help}>
	{count}
</div>

<style lang="postcss">
	.badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		height: var(--size-14);
		min-width: var(--size-14);
		border-radius: var(--size-14);
		padding: 0 var(--size-4);
		color: var(--clr-scale-ntrl-100);
		background-color: var(--clr-scale-ntrl-40);
		line-height: 90%;
	}
</style>
