<script lang="ts">
	import BranchItem from './BranchItem.svelte';
	import BranchesHeader from './BranchesHeader.svelte';
	import FilterPopupMenu from '$lib/components/FilterPopupMenu.svelte';
	import ImgThemed from '$lib/components/ImgThemed.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import { storeToObservable } from '$lib/rxjs/store';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { BehaviorSubject, combineLatest } from 'rxjs';
	import { getContext, onDestroy, onMount } from 'svelte';
	import { derived } from 'svelte/store';
	import type { BranchService } from '$lib/branches/service';
	import type { CombinedBranch } from '$lib/branches/types';
	import type { GitHubService } from '$lib/github/service';

	export let branchService: BranchService;
	export let githubService: GitHubService;
	export let projectId: string;

	export const textFilter$ = new BehaviorSubject<string | undefined>(undefined);

	const githubEnabled$ = githubService.isEnabled$;
	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const height = persisted<number | undefined>(undefined, 'branchesHeight');

	let includePrs = persisted(true, 'includePrs_' + projectId);
	let includeRemote = persisted(true, 'includeRemote_' + projectId);
	let includeStashed = persisted(true, 'includeStashed_' + projectId);
	let hideBots = persisted(false, 'hideBots_' + projectId);
	let hideInactive = persisted(false, 'hideInactive_' + projectId);

	let filtersActive = derived(
		[includePrs, includeRemote, includeStashed, hideBots, hideInactive],
		([prs, remote, stashed, bots, inactive]) => {
			return !prs || !remote || !stashed || bots || inactive;
		}
	);

	$: branches$ = branchService.branches$;
	$: filteredBranches$ = combineLatest(
		[
			branches$,
			textFilter$,
			storeToObservable(includePrs),
			storeToObservable(includeRemote),
			storeToObservable(includeStashed),
			storeToObservable(hideBots),
			storeToObservable(hideInactive)
		],
		(branches, search, includePrs, includeRemote, includeStashed, hideBots, hideInactive) => {
			const filteredByType = filterByType(branches, {
				includePrs,
				includeRemote,
				includeStashed,
				hideBots
			});
			const filteredBySearch = filterByText(filteredByType, search);
			return hideInactive ? filterInactive(filteredBySearch) : filteredBySearch;
		}
	);

	let resizeGuard: HTMLElement;
	let viewport: HTMLDivElement;
	let rsViewport: HTMLElement;
	let contents: HTMLElement;

	let observer: ResizeObserver;
	let maxHeight: number;

	function filterByType(
		branches: CombinedBranch[],
		params: {
			includePrs: boolean;
			includeRemote: boolean;
			includeStashed: boolean;
			hideBots: boolean;
		}
	): CombinedBranch[] {
		return branches.filter((b) => {
			if (params.includePrs && b.pr) {
				return !params.hideBots || !b.pr.author?.isBot;
			}
			if (params.includeRemote && b.remoteBranch) return true;
			if (params.includeStashed && b.vbranch) return true;
			return false;
		});
	}

	function filterByText(branches: CombinedBranch[], search: string | undefined) {
		if (search == undefined) return branches;

		return branches.filter((b) => searchMatchesAnIdentifier(search, b.searchableIdentifiers));
	}

	function searchMatchesAnIdentifier(search: string, identifiers: string[]) {
		for (const identifier of identifiers) {
			if (identifier.includes(search.toLowerCase())) return true;
		}

		return false;
	}

	function filterInactive(branches: CombinedBranch[]) {
		const currentTs = new Date().getTime();
		return branches.filter((b) => {
			if (!b.modifiedAt) return true; // Keep things that don't have a modified time

			const modifiedAt = b.modifiedAt?.getTime();
			const ms = currentTs - modifiedAt;
			return ms < 14 * 86400 * 1000;
		});
	}

	function updateResizable() {
		if (resizeGuard) {
			maxHeight = resizeGuard.offsetHeight / (16 * $userSettings.zoom);
		}
	}

	onMount(() => {
		updateResizable();
		observer = new ResizeObserver(() => updateResizable());
		if (viewport) observer.observe(resizeGuard);

		// Set explicit height if not found in storage. In practice this means
		// that the height is by default maximised, and won't shift when filters
		// are applied/unapplied.
		if (!$height && maxHeight) {
			$height = maxHeight;
		}
	});

	onDestroy(() => observer.disconnect());
</script>

<div class="resize-guard" bind:this={resizeGuard}>
	<div
		class="branch-list"
		bind:this={rsViewport}
		style:height={$height ? `${$height}rem` : undefined}
		style:max-height={maxHeight ? `${maxHeight}rem` : undefined}
	>
		<Resizer
			viewport={rsViewport}
			direction="up"
			inside
			defaultLineColor="var(--clr-theme-container-outline-light)"
			minHeight={90}
			on:height={(e) => {
				$height = Math.min(maxHeight, e.detail / (16 * $userSettings.zoom));
			}}
		/>
		<BranchesHeader count={$filteredBranches$?.length ?? 0} filtersActive={$filtersActive}>
			<FilterPopupMenu
				slot="context-menu"
				let:visible
				{visible}
				{includePrs}
				{includeRemote}
				{includeStashed}
				{hideBots}
				{hideInactive}
				showPrCheckbox={$githubEnabled$}
				on:action
			/>
		</BranchesHeader>
		{#if $branches$?.length > 0}
			<ScrollableContainer bind:viewport showBorderWhenScrolled>
				<div class="scroll-container">
					<TextBox
						icon="filter"
						placeholder="Search"
						on:input={(e) => textFilter$.next(e.detail)}
					/>
					<div bind:this={contents} class="content">
						{#each $filteredBranches$ as branch}
							<BranchItem {projectId} {branch} />
						{/each}
					</div>
				</div>
			</ScrollableContainer>
		{:else if $branches$.length > 0}
			<div class="branch-list__empty-state">
				<div class="branch-list__empty-state__image">
					<ImgThemed
						imgSet={{
							light: '/images/no-branches-light.webp',
							dark: '/images/no-branches-dark.webp'
						}}
					/>
				</div>
				<span class="branch-list__empty-state__caption text-base-body-14 text-semibold"
					>No branches match your filter</span
				>
			</div>
		{:else}
			<div class="branch-list__empty-state">
				<div class="branch-list__empty-state__image">
					<ImgThemed
						imgSet={{
							light: '/images/no-branches-light.webp',
							dark: '/images/no-branches-dark.webp'
						}}
					/>
				</div>
				<span class="branch-list__empty-state__caption text-base-body-14 text-semibold"
					>You have no branches</span
				>
			</div>
		{/if}
	</div>
</div>

<style lang="postcss">
	.resize-guard {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		justify-content: flex-end;
		position: relative;
		overflow-y: hidden;
	}
	.scroll-container {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
		width: 100%;
		padding-bottom: var(--space-16);
		padding-left: var(--space-12);
		padding-right: var(--space-12);
	}
	.branch-list {
		position: relative;
		display: flex;
		flex-direction: column;
	}
	.content {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		gap: var(--space-2);
	}

	/* EMPTY STATE */
	.branch-list__empty-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		gap: var(--space-10);
	}

	.branch-list__empty-state__image {
		width: 8.125rem;
	}

	.branch-list__empty-state__caption {
		color: var(--clr-theme-scale-ntrl-60);
		text-align: center;
		max-width: 10rem;
	}
</style>
