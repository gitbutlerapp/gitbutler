<script lang="ts">
	import StackTabMenu from './StackTabMenu.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import StackTab from '$components/v3/StackTab.svelte';
	import StackTabNew from '$components/v3/StackTabNew.svelte';
	import { toggleBoolQueryParam, stackPath } from '$lib/routes/routes.svelte';
	import { IdSelection } from '$lib/selection/idSelection.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';
	import { page } from '$app/state';

	type Props = {
		projectId: string;
		selectedId?: string;
		previewing: boolean;
		width: number | undefined;
	};
	let { projectId, selectedId, width = $bindable() }: Props = $props();

	const [stackService, idSelection] = inject(StackService, IdSelection);
	const result = $derived(stackService.stacks(projectId));
	const selection = $derived(idSelection.values());

	let tabs = $state<HTMLDivElement>();
	let scroller = $state<HTMLDivElement>();

	let scrollable = $state(false);
	let scrolled = $state(false);
	let scrolledEnd = $state(false);

	function onscroll() {
		scrolled = scroller && scroller.scrollLeft > 0 ? true : false;
		scrolledEnd = scroller
			? scroller.scrollLeft + scroller.offsetWidth >= scroller.scrollWidth
			: false;
	}

	onMount(() => {
		const observer = new ResizeObserver(() => {
			scrollable = scroller ? scroller.scrollWidth > scroller.offsetWidth : false;
			width = tabs?.offsetWidth;
		});
		observer.observe(tabs!);
		return () => {
			observer.disconnect();
		};
	});
</script>

<div class="tabs" bind:this={tabs}>
	<div class="inner">
		<div class="scroller" bind:this={scroller} class:scrolled {onscroll}>
			{#if selection.length > 0}
				<StackTab
					href={toggleBoolQueryParam('preview')}
					name="Preview"
					first
					selected={page.url.searchParams.has('preview')}
				/>
			{/if}
			<ReduxResult result={result.current}>
				{#snippet children(result)}
					{#if result.length > 0}
						{#each result as tab, i (tab.branchNames[0])}
							{@const last = i === result.length - 1}
							{@const selected = tab.id === selectedId}
							<StackTab
								name={tab.branchNames[0]!}
								href={stackPath(projectId, tab.id)}
								anchors={tab.branchNames.slice(1)}
								{last}
								{selected}
							>
								{#snippet menu()}
									<StackTabMenu />
								{/snippet}
							</StackTab>
						{/each}
					{:else}
						no stacks
					{/if}
				{/snippet}
			</ReduxResult>
		</div>
		<div class="shadow shadow-left" class:scrolled></div>
		<div class="shadow shadow-right" class:scrollable class:scrolled-end={scrolledEnd}></div>
	</div>
	<StackTabNew {projectId} stackId={selectedId} />
</div>

<style lang="postcss">
	.tabs {
		display: flex;
		max-width: 100%;
		width: fit-content;
	}

	.scroller {
		display: flex;
		overflow-x: scroll;
		scroll-snap-type: x proximity;
		scroll-behavior: smooth;
	}

	.scroller::-webkit-scrollbar {
		display: none;
	}

	.inner {
		position: relative;
		overflow-x: hidden;
		border-radius: 10px 0 0 0;
		border-left: 1px solid var(--clr-border-2);
		border-top: 1px solid var(--clr-border-2);
	}

	.shadow {
		position: absolute;
		top: 0;
		height: 100%;
		width: 12px;
	}

	.shadow-left {
		pointer-events: none;
		opacity: 0;
		left: 0;
		background: linear-gradient(
			to right,
			var(--clr-bg-3) 0%,
			oklch(from var(--clr-bg-3) l c h / 0) 100%
		);
		transition: opacity var(--transition-fast);

		&.scrolled {
			opacity: 1;
		}
	}

	.shadow-right {
		pointer-events: none;
		opacity: 0;
		right: 0;
		background: linear-gradient(
			to left,
			var(--clr-bg-3) 0%,
			oklch(from var(--clr-bg-3) l c h / 0) 100%
		);
		transition: opacity var(--transition-fast);

		&.scrollable {
			opacity: 1;

			&.scrolled-end {
				opacity: 0;
			}
		}
	}
</style>
