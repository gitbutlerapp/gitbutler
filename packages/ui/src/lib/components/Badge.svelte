<script lang="ts">
	export let count: string | number;
</script>

<div class="badge text-base-10 text-bold">
	{count}
</div>

<style lang="postcss">
	.badge {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		text-align: center;
		height: var(--space-16);
		min-width: var(--space-16);
		border-radius: var(--space-16);
		padding: 0 var(--space-4);
		color: var(--clr-theme-scale-ntrl-30);
		background-color: color-mix(in srgb, var(--clr-theme-scale-ntrl-40) 16%, transparent);
		line-height: 90%;
	}
</style>
