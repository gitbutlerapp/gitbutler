<script lang="ts">
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import type { BranchService } from '$lib/branches/service';
	import type { UIEventHandler } from 'svelte/elements';
	import BranchItem from './BranchItem.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import { onDestroy, onMount } from 'svelte';
	import SectionHeader from './SectionHeader.svelte';
	import { accordion } from './accordion';
	import BranchFilter, { type TypeFilter } from './BranchFilter.svelte';
	import { BehaviorSubject, combineLatest } from 'rxjs';
	import type { CombinedBranch } from '$lib/branches/types';
	import { persisted } from '@square/svelte-store';

	export let branchService: BranchService;
	export let projectId: string;
	export let expanded = false;

	export const textFilter$ = new BehaviorSubject<string | undefined>(undefined);
	export const typeFilter$ = new BehaviorSubject<TypeFilter>('all');

	const height = persisted<number | undefined>(undefined, 'branchesHeight_' + projectId);

	$: branches$ = branchService.branches$;
	$: filteredBranches$ = combineLatest(
		[branches$, typeFilter$, textFilter$],
		(branches, type, search) => searchFilter(typeFilter(branches, type), search)
	);

	let resizeGuard: HTMLElement;
	let viewport: HTMLElement;
	let rsViewport: HTMLElement;
	let contents: HTMLElement;

	let observer: ResizeObserver;
	let maxHeight: number;

	let scrolled: boolean;
	const onScroll: UIEventHandler<HTMLDivElement> = (e) => {
		scrolled = e.currentTarget.scrollTop != 0;
	};

	function typeFilter(branches: CombinedBranch[], type: TypeFilter): CombinedBranch[] {
		switch (type) {
			case 'all':
				return branches;
			case 'vbranch':
				return branches.filter((b) => b.vbranch);
			case 'branch':
				return branches.filter((b) => b.remoteBranch);
			case 'pr':
				return branches.filter((b) => b.pr);
		}
	}

	function searchFilter(branches: CombinedBranch[], search: string | undefined) {
		if (search == undefined) return branches;
		return branches.filter((b) => b.displayName.includes(search));
	}

	function updateResizable() {
		if (resizeGuard) {
			maxHeight = resizeGuard.offsetHeight / 16;
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
		style:height={expanded && $height ? `${$height}rem` : undefined}
		style:max-height={maxHeight ? `${maxHeight}rem` : undefined}
	>
		{#if expanded}
			<Resizer
				viewport={rsViewport}
				direction="up"
				inside
				minHeight={90}
				on:height={(e) => {
					$height = Math.min(maxHeight, e.detail / 16);
				}}
			/>
		{/if}
		<SectionHeader {scrolled} count={$branches$?.length ?? 0} expandable={true} bind:expanded>
			Branches
		</SectionHeader>
		<div class="scroll-container" use:accordion={$branches$?.length > 0 && expanded}>
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
		position: relative;
		overflow: hidden;
	}
	.branch-list {
		position: relative;
		display: flex;
		flex-direction: column;
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
