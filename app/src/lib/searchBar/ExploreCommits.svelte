<script lang="ts">
	import { getFilterContext } from './filterContext.svelte';
	import Badge from '$lib/shared/Badge.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import { getContext } from '$lib/utils/context';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';
	import {
		FilterCatergoryValue,
		FilterName,
		formatFilterValues,
		getCommitCategoryEmoji,
		getFilterEmoji,
		isFilterCatergoryValue,
		type AppliedFilterInfo,
		type FilterDescription,
		type FilterSuggestion
	} from '$lib/vbranches/filtering';

	const DYNAMIC_SUGGESTIONS_QUICK_FILTER = 2;
	const DYNAMIC_SUGGESTIONS_EXPANDED_FILTER = 10;

	interface Props {
		expanded: boolean;
		filterDescriptions: FilterDescription[];
	}

	let { expanded = $bindable(), filterDescriptions }: Props = $props();

	const filterContext = getFilterContext();
	const baseBranchService = getContext(BaseBranchService);

	let isBusy = $state<boolean>(true);
	baseBranchService.busy$.subscribe((busy) => (isBusy = busy));

	const quickFilters = $derived<FilterSuggestion[]>(
		filterDescriptions
			.flatMap(
				(d) => d.dynamicSuggestions?.slice(undefined, DYNAMIC_SUGGESTIONS_QUICK_FILTER) ?? []
			)
			.filter((s) => !filterContext.hasRecentFilter(s))
	);

	const filterDescriptionPairs = $derived.by<FilterDescription[][]>(() => {
		const pairs = [];
		for (let i = 0; i < filterDescriptions.length; i += 2) {
			pairs.push(filterDescriptions.slice(i, i + 2));
		}
		return pairs;
	});

	const commitCategories = $derived<FilterCatergoryValue[]>(
		filterDescriptions
			.filter((d) => d.name === FilterName.Category)
			.flatMap(
				(d) => d.suggestions?.map((s) => s.value).filter((v) => isFilterCatergoryValue(v)) ?? []
			)
	);

	function handleSuggestionClick(suggestion: FilterSuggestion) {
		addSuggestion: {
			if (suggestion.value === undefined) break addSuggestion;
			filterContext.addFilter({ name: suggestion.name, values: [suggestion.value] });
		}
		expanded = false;
	}

	function handleFilterClick(filter: AppliedFilterInfo) {
		filterContext.addFilter(filter);
		expanded = false;
	}
</script>

<div class="explore-container" class:expanded>
	<div class="explore-row">
		{#each filterContext.recentFilters as filter}
			<button
				onclick={() => handleFilterClick(filter)}
				class="explore-filter card text-semibold text-base-11"
			>
				{getFilterEmoji(filter.name)}
				{formatFilterValues(filter)}
			</button>
		{/each}
		{#each quickFilters as filter}
			<button
				onclick={() => handleSuggestionClick(filter)}
				class="explore-filter card text-semibold text-base-11"
			>
				{getFilterEmoji(filter.name)}
				{filter.value}
			</button>
		{/each}
		<button
			class="explore-filter card text-semibold text-base-11 fixed"
			class:hidden={expanded}
			onclick={() => (expanded = true)}
		>
			more...
		</button>
	</div>

	<div class="explore-row center" class:hidden={!expanded}>
		<button
			class="explore-filter card text-semibold text-base-11"
			onclick={() => (expanded = false)}
		>
			📋 show the commit list
		</button>
	</div>

	{#if commitCategories.length}
		<div class="transition-fly explore-row" class:hidden={!expanded}>
			<div class="card explore-list single-line">
				<h3 class="text-base-18 text-semibold">
					{getFilterEmoji(FilterName.Category)} Categories
					{#if isBusy}
						<Icon name="spinner" />
					{/if}
				</h3>

				<div class="category-container text-base-16">
					{#each commitCategories as category}
						<button
							onclick={() => handleFilterClick({ name: FilterName.Category, values: [category] })}
						>
							{getCommitCategoryEmoji(category)}
							{category}
						</button>
					{/each}
				</div>
			</div>
		</div>
	{/if}

	<div class="transition-fly explore-row" class:hidden={!expanded}>
		{#each filterDescriptionPairs as filterPair}
			{#each filterPair as filter}
				{#if filter.dynamicSuggestions?.length}
					<div class="card explore-list">
						<h3 class="text-base-14 text-semibold">
							{getFilterEmoji(filter.name)} Top {filter.name}s
							{#if isBusy}
								<Icon name="spinner" />
							{/if}
						</h3>
						<ul>
							{#each filter.dynamicSuggestions.slice(undefined, DYNAMIC_SUGGESTIONS_EXPANDED_FILTER) as suggestion}
								<li class="text-base-12">
									<button onclick={() => handleSuggestionClick(suggestion)}>
										<div class="dynamic-sugesstion">
											{suggestion.value}
											<Badge count={suggestion.metric.value} />
										</div>
									</button>
								</li>
							{/each}
						</ul>
					</div>
				{/if}
			{/each}
		{/each}
	</div>
</div>

<style lang="postcss">
	.explore-container {
		z-index: var(--z-ground);
		display: flex;
		flex-direction: column;
		overflow: hidden;
		padding-bottom: 0;
		box-sizing: border-box;
		width: 100%;
		flex-shrink: 0;
		align-items: start;
		justify-content: start;
		padding: 12px;
		padding-bottom: 0;

		&.expanded {
			gap: 12px;
			padding-bottom: unset;
		}
	}

	.explore-row {
		display: flex;
		flex-wrap: nowrap;
		overflow-x: auto;
		overflow-y: hidden;
		gap: 12px;
		box-sizing: border-box;
		width: 100%;
		transition:
			opacity var(--transition-slower),
			height var(--transition-slower);

		&.center {
			justify-content: center;
		}

		&::-webkit-scrollbar {
			display: none;
		}

		&.hidden {
			opacity: 0;
			height: 0;
			margin: 0;
		}
	}

	.explore-list {
		width: 100%;
		min-height: 278px;
		display: flex;
		flex-direction: column;
		gap: 6px;

		&.single-line {
			min-height: unset;
		}

		& li > button {
			width: 100%;
			text-align: left;
			white-space: nowrap;
			text-overflow: ellipsis;
			overflow: hidden;
			border-radius: var(--radius-m);
			padding: 4px 8px;
			transition: background var(--transition-fast);

			&:hover {
				background: var(--clr-theme-ntrl-soft-hover);
			}
		}

		& .category-container {
			padding: 6px 0;
			& button {
				text-align: left;
				white-space: nowrap;
				text-overflow: ellipsis;
				overflow: hidden;
				border-radius: var(--radius-m);
				padding: 4px 8px;
				margin: 4px 4px;
				transition: background var(--transition-fast);
				border: 1px solid var(--clr-border-2);

				&:hover {
					background: var(--clr-theme-ntrl-soft-hover);
				}
			}
		}
	}

	.dynamic-sugesstion {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.explore-filter {
		display: flex;
		align-items: center;
		transition:
			opacity var(--transition-slower),
			background var(--transition-fast);
		flex-grow: 0;
		flex-shrink: 0;
		color: var(--btn-text-clr);

		&:hover {
			background: var(--clr-theme-ntrl-soft-hover);
		}

		&.hidden {
			opacity: 0;
			pointer-events: none;
		}

		&.fixed {
			position: sticky;
			right: 12px;
			box-shadow: var(--fx-shadow-s);
		}
	}

	.card {
		padding: 16px;
	}
</style>
