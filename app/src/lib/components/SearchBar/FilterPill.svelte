<script lang="ts">
	interface Props {
		name: string;
		value: string | undefined;
	}
	let { name, value }: Props = $props();
</script>

<div class="filter-pill">
	<span class="filter-name-prop">{name}</span>
	{#if value}
		<span class="filter-name-value">{value}</span>
	{/if}
</div>

<style>
	.filter-pill {
		box-sizing: border-box;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: var(--clr-scale-ntrl-70);
		display: flex;
		padding: 2px 4px;
	}

	.filter-name-prop {
		color: var(--clr-scale-ntrl-30);
	}

	.filter-name-value {
		color: var(--clr-scale-ntrl-0);
	}
</style>
