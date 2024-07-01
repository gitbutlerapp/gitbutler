<script lang="ts">
	import { getFilterContext } from './filterContext.svelte';
	import { Project } from '$lib/backend/projects';
	import BranchIcon from '$lib/branch/BranchIcon.svelte';
	import { BranchService } from '$lib/branches/service';
	import Badge from '$lib/shared/Badge.svelte';
	import Icon from '$lib/shared/Icon.svelte';
	import ScrollableContainer from '$lib/shared/ScrollableContainer.svelte';
	import { getRelevantRemoteBranchData } from '$lib/stores/remoteBranches';
	import { getBranchLink } from '$lib/utils/branch';
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
	import type { CombinedBranch } from '$lib/branches/types';
	import type { Branch, RemoteBranch } from '$lib/vbranches/types';
	import { goto } from '$app/navigation';

	const DYNAMIC_SUGGESTIONS_QUICK_FILTER = 2;
	const DYNAMIC_SUGGESTIONS_EXPANDED_FILTER = 10;

	interface Props {
		expanded: boolean;
		filterDescriptions: FilterDescription[];
		remoteBranchNames: string[];
		activeBranches: Branch[] | undefined;
	}

	let {
		expanded = $bindable(),
		filterDescriptions,
		remoteBranchNames,
		activeBranches
	}: Props = $props();

	const filterContext = getFilterContext();
	const baseBranchService = getContext(BaseBranchService);
	const project = getContext(Project);
	const branchService = getContext(BranchService);

	let isBusy = $state<boolean>(true);
	baseBranchService.busy$.subscribe((busy) => (isBusy = busy));

	let combinedBranches = $state<CombinedBranch[]>([]);
	branchService.branches$.subscribe((b) => (combinedBranches = b));

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

	const filesBeingWorkedOn = $derived<string[]>(
		activeBranches?.flatMap((b) => b.files.map((f) => f.path)) ?? []
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

	function getBranchIcon(remoteBranch: RemoteBranch) {
		return combinedBranches.find((b) => b.remoteBranch?.name === remoteBranch.name)?.icon;
	}

	function handleBranchActivityClick(remoteBranch: RemoteBranch) {
		const combinedBranch = combinedBranches.find((b) => b.remoteBranch?.name === remoteBranch.name);
		const href = combinedBranch && getBranchLink(combinedBranch, project.id);
		if (href) goto(href);
	}
</script>

<div class="explore-container" class:expanded>
	<div class="explore-row">
		{#each filterContext.recentFilters as filter}
			<button
				onclick={() => handleFilterClick(filter)}
				class="card explore-filter text-semibold text-base-11"
			>
				{getFilterEmoji(filter.name)}
				{formatFilterValues(filter)}
			</button>
		{/each}
		{#each quickFilters as filter}
			<button
				onclick={() => handleSuggestionClick(filter)}
				class="card explore-filter text-semibold text-base-11"
			>
				{getFilterEmoji(filter.name)}
				{filter.value}
			</button>
		{/each}
		<button
			class="card explore-filter text-semibold text-base-11 fixed"
			class:hidden={expanded}
			onclick={() => (expanded = true)}
		>
			more...
		</button>
	</div>

	<div class="explore-row center" class:hidden={!expanded}>
		<button
			class="card explore-filter text-semibold text-base-11"
			onclick={() => (expanded = false)}
		>
			📋 show the commit list
		</button>
	</div>

	{#if remoteBranchNames.length}
		<div class="transition-fly explore-row" class:hidden={!expanded}>
			<div class="card explore-list" style:height="200px">
				<h3 class="text-base-18 text-semibold">Activity ⚡️</h3>
				{#if filesBeingWorkedOn.length === 0}
					<div class="center">
						<p class="text-base-14">
							No files are being currently worked on. This will update once you make some local
							changes 😄
						</p>
					</div>
				{:else}
					<ScrollableContainer wide>
						<div class="explore-list-container">
							{#each filesBeingWorkedOn as filePath}
								<div class="explore-list-container">
									<h4 class="text-base-14 text-semibold">on {filePath}</h4>
									<ul>
										{#each remoteBranchNames as bName}
											{#await getRelevantRemoteBranchData(project.id, bName, filePath) then branchData}
												{#if branchData}
													<li class="transition-fly">
														<button
															onclick={() => handleBranchActivityClick(branchData.remoteBranch)}
														>
															<div class="text-base-11 explore-list-item gapped">
																<div>
																	<BranchIcon
																		help={undefined}
																		name={getBranchIcon(branchData.remoteBranch)}
																	/>
																</div>
																<div>
																	<span class="text-semibold">
																		{branchData.remoteBranch.displayName}:
																	</span>
																	{branchData.commit.author.name} is working on
																	<span class="text-semibold">
																		{branchData.commit.descriptionTitle}ed2d23d2323d23d2
																	</span>
																</div>
															</div>
														</button>
													</li>
												{/if}
											{/await}
										{/each}
									</ul>
								</div>
							{/each}
						</div>
					</ScrollableContainer>
				{/if}
			</div>
		</div>
	{/if}

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
										<div class="explore-list-item space-between">
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

		& .center {
			display: flex;
			justify-content: center;
			align-items: center;
			height: 100%;
		}
	}

	.explore-list-container {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.explore-list-item {
		display: flex;
		align-items: center;

		&.space-between {
			justify-content: space-between;
		}

		&.gapped {
			gap: 6px;
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
		padding: 4px 8px;

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
