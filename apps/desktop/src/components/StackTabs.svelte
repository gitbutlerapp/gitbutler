<script lang="ts">
	import ReduxResult from './ReduxResult.svelte';
	import StackTab from './StackTab.svelte';
	import StackTabNew from './StackTabNew.svelte';
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
	const result = $derived(stackService.getStacks(projectId));

	let inner = $state<HTMLDivElement>();
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
			width = inner?.offsetWidth;
		});
		observer.observe(inner!);
		return () => {
			observer.disconnect();
		};
	});
</script>

<div class="tabs">
	<div class="inner" bind:this={inner}>
		<div class="shadows">
			<div class="scroller" bind:this={scroller} class:scrolled {onscroll}>
				<ReduxResult result={result.current}>
					{#snippet children(result)}
						{@const tabs = stacksToTabs(result)}
						{#each tabs as tab, i (tab.name)}
							{@const first = i === 0}
							{@const last = i === tabs.length - 1}
							{@const selected = tab.id === selectedId}
							<StackTab {projectId} {tab} {first} {last} {selected} />
						{/each}
					{/snippet}
					{#snippet empty()}
						no stacks
					{/snippet}
				</ReduxResult>
			</div>
			<div class="shadow shadow-left" class:scrolled></div>
			<div class="shadow shadow-right" class:scrollable class:scrolled-end={scrolledEnd}></div>
		</div>
		<StackTabNew {projectId} />
	</div>
</div>

<style lang="postcss">
	.tabs {
		display: flex;
	}
	.inner {
		display: flex;
		max-width: 100%;
	}

	.scroller {
		display: flex;
		overflow-x: scroll;
	}

	.scroller::-webkit-scrollbar {
		display: none;
	}

	.shadows {
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
		width: 8px;
	}

	.shadow-left {
		display: none;
		left: 0;
		background: linear-gradient(to right, rgba(0, 0, 0, 0.2), rgba(0, 0, 0, 0));
		&.scrolled {
			display: block;
		}
	}

	.shadow-right {
		display: none;
		right: 0;
		background: linear-gradient(to left, rgba(0, 0, 0, 0.2), rgba(0, 0, 0, 0));
		&.scrollable {
			display: initial;
			&.scrolled-end {
				display: none;
			}
		}
	}
</style>
