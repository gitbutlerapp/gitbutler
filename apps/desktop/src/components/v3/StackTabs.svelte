<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import StackTab from '$components/v3/StackTab.svelte';
	import StackTabNew from '$components/v3/StackTabNew.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { stacksToTabs } from '$lib/tabs/mapping';
	import { getContext } from '@gitbutler/shared/context';
	import { onMount } from 'svelte';

	type Props = {
		projectId: string;
		selectedId: string | undefined;
		previewing: boolean;
		width: number | undefined;
	};
	let { projectId, selectedId, width = $bindable() }: Props = $props();

	const stackService = getContext(StackService);
	const result = $derived(stackService.stacks(projectId));

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
			<ReduxResult result={result.current}>
				{#snippet children(result)}
					{#if result.length > 0}
						{@const tabs = stacksToTabs(result)}
						{#each tabs as tab, i (tab.name)}
							{@const first = i === 0}
							{@const last = i === tabs.length - 1}
							{@const selected = tab.id === selectedId}
							<StackTab {projectId} {tab} {first} {last} {selected} />
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
	<StackTabNew {projectId} />
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
