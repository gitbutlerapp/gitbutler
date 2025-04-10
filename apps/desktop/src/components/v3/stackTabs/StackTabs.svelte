<script lang="ts">
	import StackTab from '$components/v3/stackTabs/StackTab.svelte';
	import StackTabDraft from '$components/v3/stackTabs/StackTabDraft.svelte';
	import StackTabNew from '$components/v3/stackTabs/StackTabNew.svelte';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { onMount } from 'svelte';
	import type { Stack } from '$lib/stacks/stack';

	type Props = {
		projectId: string;
		stacks: Stack[];
		selectedId?: string;
		isCommitting: boolean;
		width: number | undefined;
	};
	let { projectId, stacks, selectedId, isCommitting, width = $bindable() }: Props = $props();

	let plusBtnEl = $state<HTMLButtonElement>();
	let tabsEl = $state<HTMLDivElement>();
	let scrollerEl = $state<HTMLDivElement>();

	let scrollable = $state(false);
	let scrolled = $state(false);
	let scrolledEnd = $state(false);

	function onscroll() {
		scrolled = scrollerEl && scrollerEl.scrollLeft > 0 ? true : false;
		scrolledEnd = scrollerEl
			? scrollerEl.scrollLeft + scrollerEl.offsetWidth >= scrollerEl.scrollWidth
			: false;
	}

	onMount(() => {
		const observer = new ResizeObserver(() => {
			scrollable = scrollerEl ? scrollerEl.scrollWidth > scrollerEl.offsetWidth : false;
			width = tabsEl?.offsetWidth;
		});
		observer.observe(tabsEl!);
		return () => {
			observer.disconnect();
		};
	});
</script>

<div class="tabs" bind:this={tabsEl}>
	{#if stacks.length > 0}
		<div class="inner">
			<div class="scroller" bind:this={scrollerEl} class:scrolled {onscroll}>
				{#each stacks as stack, i (stack.branchNames[0])}
					{@const first = i === 0}
					{@const last = i === stacks.length - 1}
					{@const selected = stack.id === selectedId}

					<StackTab
						name={stack.branchNames[0]!}
						{projectId}
						stackId={stack.id}
						href={stackPath(projectId, stack.id)}
						anchors={stack.branchNames.slice(1)}
						{selected}
						onNextTab={() => {
							if (last) {
								plusBtnEl?.focus();
							}
						}}
						onPrevTab={() => {
							if (first) {
								plusBtnEl?.focus();
							}
						}}
					/>
				{/each}
			</div>
			<div class="shadow shadow-left" class:scrolled></div>
			<div class="shadow shadow-right" class:scrollable class:scrolled-end={scrolledEnd}></div>
		</div>
	{/if}
	{#if isCommitting && stacks.length === 0}
		<StackTabDraft />
	{:else}
		<StackTabNew
			bind:el={plusBtnEl}
			{scrollerEl}
			{projectId}
			stackId={selectedId}
			noStacks={stacks.length === 0}
		/>
	{/if}
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
