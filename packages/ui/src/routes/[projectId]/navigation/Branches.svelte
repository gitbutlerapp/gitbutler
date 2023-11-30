<script lang="ts">
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import type { BranchService } from '$lib/branches/service';
	import type { UIEventHandler } from 'svelte/elements';
	import BranchItem from './BranchItem.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import { getContext } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import SectionHeader from './SectionHeader.svelte';
	import { accordion } from './accordion';
	import BranchFilter, { type TypeFilter } from './BranchFilter.svelte';
	import { BehaviorSubject, combineLatest } from 'rxjs';
	import type { CombinedBranch } from '$lib/branches/types';

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	export let branchService: BranchService;
	export let projectId: string;
	export let expanded = false;

	export const textFilter$ = new BehaviorSubject<string | undefined>(undefined);
	export const typeFilter$ = new BehaviorSubject<TypeFilter>('all');

	$: branches$ = branchService.branches$;
	$: filteredBranches$ = combineLatest(
		[branches$, typeFilter$, textFilter$],
		(branches, type, search) => searchFilter(typeFilter(branches, type), search)
	);

	let viewport: HTMLElement;
	let contents: HTMLElement;

	let scrolled: boolean;
	const onScroll: UIEventHandler<HTMLDivElement> = (e) => {
		scrolled = e.currentTarget.scrollTop != 0;
	};

	function typeFilter(branches: CombinedBranch[], type: TypeFilter): CombinedBranch[] {
		switch (type) {
			case 'all':
				return branches;
			case 'branch':
				return branches.filter((b) => b.branch && !b.pr);
			case 'pr':
				return branches.filter((b) => b.pr);
		}
	}

	function searchFilter(branches: CombinedBranch[], search: string | undefined) {
		if (search == undefined) return branches;
		return branches.filter((b) => b.displayName.includes(search));
	}
</script>

<div class="relative flex flex-col">
	{#if expanded}
		<Resizer
			{viewport}
			direction="up"
			inside
			minHeight={90}
			on:height={(e) => {
				userSettings.update((s) => ({
					...s,
					vbranchExpandableHeight: e.detail
				}));
			}}
		/>
	{/if}
	<SectionHeader {scrolled} count={$branches$?.length ?? 0} expandable={true} bind:expanded>
		Other branches
	</SectionHeader>
	<div
		class="wrapper"
		use:accordion={$branches$?.length > 0 && expanded}
		style:height={`${$userSettings.vbranchExpandableHeight}px`}
	>
		<div bind:this={viewport} class="viewport hide-native-scrollbar" on:scroll={onScroll}>
			<BranchFilter {typeFilter$} {textFilter$}></BranchFilter>
			<div bind:this={contents} class="content">
				{#if $filteredBranches$}
					{#each $filteredBranches$ as branch}
						<BranchItem {projectId} {branch} />
					{/each}
				{/if}
			</div>
		</div>
		<Scrollbar {viewport} {contents} thickness="0.5rem" />
	</div>
</div>

<style lang="postcss">
	.wrapper {
		position: relative;
		overflow: hidden;
	}
	.viewport {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
		height: 100%;
		overflow-y: scroll;
		overscroll-behavior: none;
		padding-top: var(--space-4);
		padding-bottom: var(--space-16);
		padding-left: var(--space-12);
		padding-right: var(--space-12);
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
	}
</style>
