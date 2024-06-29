<script lang="ts">
	import { getFilterContext } from './filterContext.svelte';
	import {
		getFilterEmoji,
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

	const quickFilters = filterDescriptions.flatMap(
		(d) => d.dynamicSuggestions?.slice(undefined, DYNAMIC_SUGGESTIONS_QUICK_FILTER) ?? []
	);

	function handleSuggestionClick(suggestion: FilterSuggestion) {
		addSuggestion: {
			if (suggestion.value === undefined) break addSuggestion;
			filterContext.addFilter({ name: suggestion.name, values: [suggestion.value] });
		}
		expanded = false;
	}
</script>

<div class="explore-container" class:expanded>
	<div class="explore-row">
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

	<div class="transition-fly explore-row wrap" class:hidden={!expanded}>
		{#each filterDescriptions as filter}
			{#if filter.dynamicSuggestions?.length}
				<div class="card explore-list">
					<h3 class="text-base-14 text-semibold">
						{getFilterEmoji(filter.name)} Top {filter.name}s
					</h3>
					<ul>
						{#each filter.dynamicSuggestions.slice(undefined, DYNAMIC_SUGGESTIONS_EXPANDED_FILTER) as suggestion}
							<li class="text-base-12">
								<button onclick={() => handleSuggestionClick(suggestion)}>
									{suggestion.value}
								</button>
							</li>
						{/each}
					</ul>
				</div>
			{/if}
		{/each}
	</div>

	<div class="explore-row center" class:hidden={!expanded}>
		<button
			class="explore-filter card text-semibold text-base-11"
			onclick={() => (expanded = false)}
		>
			📋 show the commit list
		</button>
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
			margin-top: 24px;
			justify-content: center;
		}

		&::-webkit-scrollbar {
			display: none;
		}

		&.wrap {
			flex-wrap: wrap;
			justify-content: start;
			align-items: start;
		}

		&.hidden {
			opacity: 0;
			height: 0;
			margin: 0;
		}
	}

	.explore-list {
		width: 420px;
		min-height: 278px;
		display: flex;
		flex-direction: column;
		gap: 6px;

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
