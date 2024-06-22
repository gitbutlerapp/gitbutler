<script lang="ts">
	import type { AppliedFilter } from "$lib/vbranches/filtering";

  interface Props {
    appliedFilters: AppliedFilter[];
  }
  let { appliedFilters }: Props = $props();
</script>

<div class="filter-pill-container">
  {#each appliedFilters as filter}
    <div class="filter-pill text-base-14">
      <div class="filter-name-prop">{filter.name}:</div>
      <div class="filter-name-value">{filter.values.join(',')}</div>
    </div>
  {/each}
</div>

<style>
  .filter-pill-container {
		margin-right: 10px;
		display: flex;
		gap: 4px;
	}

	.filter-pill {
		box-sizing: border-box;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-s);
		background-color: var(--clr-scale-ntrl-70);
		display: flex;
		padding: 4px;
	}

	.filter-name-prop {
		color: var(--clr-scale-ntrl-30);
	}

	.filter-name-value {
		color: var(--clr-scale-ntrl-0);
	}
</style>