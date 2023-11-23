<script lang="ts">
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import type { BranchService } from '$lib/branches/service';
	import type { UIEventHandler } from 'svelte/elements';
	import BranchItem from './BranchItem.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import { getContext } from 'svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import SectionHeader from './SectionHeader.svelte';

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);

	export let branchService: BranchService;
	export let projectId: string;
	export let grow: boolean;
	$: branches$ = branchService.branches$;

	let viewport: HTMLElement;
	let contents: HTMLElement;

	let scrolled: boolean;
	const onScroll: UIEventHandler<HTMLDivElement> = (e) => {
		scrolled = e.currentTarget.scrollTop != 0;
	};
</script>

<SectionHeader {scrolled} count={$branches$?.length ?? 0}>Other branches</SectionHeader>
<div
	class="wrapper"
	style:height={`${$userSettings.vbranchExpandableHeight}px`}
	class:flex-grow={grow}
>
	<div bind:this={viewport} class="viewport hide-native-scrollbar" on:scroll={onScroll}>
		<div bind:this={contents} class="content">
			{#if $branches$}
				{#each $branches$ as branch}
					<BranchItem {projectId} {branch} />
				{/each}
			{/if}
		</div>
	</div>
	<Scrollbar {viewport} {contents} width="0.5rem" />
</div>

<Resizer
	minHeight={200}
	{viewport}
	direction="vertical"
	class="z-30"
	on:height={(e) => {
		userSettings.update((s) => ({
			...s,
			vbranchExpandableHeight: e.detail
		}));
	}}
/>

<style lang="postcss">
	.wrapper {
		min-height: 10rem;
		position: relative;
		overflow: hidden;
	}
	.viewport {
		height: 100%;
		overflow-y: scroll;
		overscroll-behavior: none;
		padding-top: var(--space-4);
		padding-bottom: var(--space-16);
		padding-left: var(--space-16);
		padding-right: var(--space-16);
	}
	.content {
		display: flex;
		flex-direction: column;
		gap: var(--space-12);
	}
</style>
