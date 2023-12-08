<script lang="ts">
	import type { BranchService } from '$lib/branches/service';
	import type { CombinedBranch } from '$lib/branches/types';

	import BranchFilter, { type TypeFilter } from './BranchFilter.svelte';
	import BranchItem from './BranchItem.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import SectionHeader from './SectionHeader.svelte';
	import ScrollableContainer from '$lib/components/ScrollableContainer.svelte';

	import { getContext, onDestroy, onMount } from 'svelte';
	import { BehaviorSubject, combineLatest } from 'rxjs';
	import { persisted } from '@square/svelte-store';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';

	export let branchService: BranchService;
	export let projectId: string;
	export let expanded = false;

	export const textFilter$ = new BehaviorSubject<string | undefined>(undefined);
	export const typeFilter$ = new BehaviorSubject<TypeFilter>('all');

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const height = persisted<number | undefined>(undefined, 'branchesHeight_' + projectId);

	$: branches$ = branchService.branches$;
	$: filteredBranches$ = combineLatest(
		[branches$, typeFilter$, textFilter$],
		(branches, type, search) => searchFilter(typeFilter(branches, type), search)
	);

	let resizeGuard: HTMLElement;
	let viewport: HTMLDivElement;
	let rsViewport: HTMLElement;
	let contents: HTMLElement;

	let observer: ResizeObserver;
	let maxHeight: number;

	let scrolled: boolean;

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
					$height = Math.min(maxHeight, e.detail / (16 * $userSettings.zoom));
				}}
			/>
		{/if}
		<SectionHeader {scrolled} count={$branches$?.length ?? 0} expandable={true} bind:expanded>
			Branches
		</SectionHeader>
		<div class="expandable" class:collapsed={$branches$?.length == 0 || !expanded}>
			<ScrollableContainer bind:scrolled bind:viewport>
				<div class="scroll-container">
					<BranchFilter {typeFilter$} {textFilter$}></BranchFilter>
					<div bind:this={contents} class="content">
						{#if $filteredBranches$}
							{#each $filteredBranches$ as branch}
								<BranchItem {projectId} {branch} />
							{/each}
						{/if}
					</div>
				</div>
			</ScrollableContainer>
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
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
		width: 100%;
		padding-top: var(--space-4);
		padding-bottom: var(--space-16);
		padding-left: var(--space-12);
		padding-right: var(--space-12);
	}

	.expandable {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		&.collapsed {
			display: none;
		}
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
