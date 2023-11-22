<script lang="ts">
	export let count: string | number;
</script>

<div class="badge text-bold text-base-11 inline-block rounded-full p-1">
	{count}
</div>

<style>
	.badge {
		background-color: var(--clr-theme-container-dim);
		color: var(--clr-theme-scale-ntrl-40);
		line-height: 90%;
		/* TODO: text-base-body-11 takes precedence over text-bold */
		font-weight: 700 !important;
	}
</style>
