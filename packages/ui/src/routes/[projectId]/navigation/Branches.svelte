<script lang="ts">
	import type { BranchService } from '$lib/branches/service';
	import type { CombinedBranch } from '$lib/branches/types';

	import BranchItem from './BranchItem.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import BranchesHeader from './BranchesHeader.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';

	import { getContext, onDestroy, onMount } from 'svelte';
	import { BehaviorSubject, combineLatest } from 'rxjs';
	import { persisted } from '@square/svelte-store';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import FilterPopupMenu from '../components/FilterPopupMenu.svelte';
	import { derived } from 'svelte/store';
	import { storeToObservable } from '$lib/rxjs/store';
	import TextBox from '$lib/components/TextBox.svelte';

	export let branchService: BranchService;
	export let projectId: string;

	export const textFilter$ = new BehaviorSubject<string | undefined>(undefined);

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const height = persisted<number | undefined>(undefined, 'branchesHeight_' + projectId);

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

	const branches$ = branchService.branches$;
	const filteredBranches$ = combineLatest(
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
			return hideInactive ? filterInactive(filteredBySearch) : filteredByType;
		}
	);

	let resizeGuard: HTMLElement;
	let viewport: HTMLDivElement;
	let rsViewport: HTMLElement;
	let contents: HTMLElement;

	let observer: ResizeObserver;
	let maxHeight: number;

	let scrolled: boolean;

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
			if (!params.includePrs && b.pr) return false;
			if (!params.includeRemote && b.remoteBranch) return false;
			if (!params.includeStashed && b.vbranch) return false;
			if (params.hideBots && b.pr?.author?.isBot) return false;
			return true;
		});
	}

	function filterByText(branches: CombinedBranch[], search: string | undefined) {
		if (search == undefined) return branches;
		return branches.filter((b) => b.displayName.includes(search));
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
			minHeight={90}
			on:height={(e) => {
				$height = Math.min(maxHeight, e.detail / (16 * $userSettings.zoom));
			}}
		/>
		<BranchesHeader {scrolled} count={$branches$?.length ?? 0} filtersActive={$filtersActive}>
			<FilterPopupMenu
				slot="context-menu"
				let:visible
				{visible}
				{includePrs}
				{includeRemote}
				{includeStashed}
				{hideBots}
				{hideInactive}
				on:action
			/>
		</BranchesHeader>
		<ScrollableContainer bind:scrolled bind:viewport>
			<div class="scroll-container">
				<TextBox icon="filter" placeholder="Search" on:input={(e) => textFilter$.next(e.detail)} />
				<div bind:this={contents} class="content">
					{#if $filteredBranches$?.length > 0}
						{#each $filteredBranches$ as branch}
							<BranchItem {projectId} {branch} />
						{/each}
					{:else if $branches$.length > 0}
						No branches match your filter
					{:else}
						You have no branches
					{/if}
				</div>
			</div>
		</ScrollableContainer>
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
		gap: var(--space-2);
	}
</style>
