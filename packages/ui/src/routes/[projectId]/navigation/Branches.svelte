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
	class="container relative overflow-hidden"
	style:height={`${$userSettings.vbranchExpandableHeight}px`}
	class:flex-grow={grow}
>
	<div
		bind:this={viewport}
		class="hide-native-scrollbar h-full overflow-y-scroll overscroll-none px-4 py-1"
		on:scroll={onScroll}
	>
		<div bind:this={contents} class="flex flex-col gap-3">
			<div class="">
				{#if $branches$}
					{#each $branches$ as branch}
						<BranchItem {projectId} {branch} />
					{/each}
				{/if}
			</div>
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

<style>
	.container {
		min-height: 10rem;
	}
</style>
