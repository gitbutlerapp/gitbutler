<script lang="ts">
	interface Props {
		name: string;
		value: string | undefined;
		onclick?: () => void;
	}
	let { name, value, onclick }: Props = $props();
</script>

{#if !!onclick}
	<button class="filter-pill clickable" {onclick}>
		<span class="filter-name-prop">{name}</span>
		{#if value}
			<span class="filter-name-value">{value}</span>
		{/if}
	</button>
{:else}
	<div class="filter-pill">
		<span class="filter-name-prop">{name}</span>
		{#if value}
			<span class="filter-name-value">{value}</span>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.filter-pill {
		box-sizing: border-box;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: var(--clr-scale-ntrl-70);
		display: flex;
		padding: 2px 4px;
		text-wrap: nowrap;
	}

	.clickable {
		cursor: pointer;
		&:hover {
			background-color: var(--clr-scale-ntrl-60);
		}
	}

	.filter-name-prop {
		color: var(--clr-scale-ntrl-30);
	}

	.filter-name-value {
		color: var(--clr-scale-ntrl-0);
	}
</style>
