<script lang="ts">
	import IconChevronLeft from '$lib/icons/IconChevronLeft.svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch, Branch, BranchData } from '$lib/vbranches/types';
	import { getContext } from 'svelte';
	import VirtualBranchPeek from './VirtualBranchPeek.svelte';
	import type { Readable } from '@square/svelte-store';
	import BaseBranchPeek from './BaseBranchPeek.svelte';
	import RemoteBranchPeek from './RemoteBranchPeek.svelte';

	export let item: Readable<BranchData | Branch | BaseBranch | undefined> | undefined;
	export let base: BaseBranch | undefined;
	export let branchController: BranchController;
	export let expanded: boolean;
	export let offsetTop: number;
	export let fullHeight = false;
	export let disabled = false;

	let dragging = false;
	let hovering = false;
	let container: HTMLDivElement;
	let addMargin = 0;

	$: offsetLeft = expanded
		? $userSettings.trayWidth
		: $userSettings.trayWidth - $userSettings.peekTrayWidth;

	export function close() {
		expanded = false;
	}

	function toggleExpanded() {
		expanded = !expanded;
		dragging = false;
		hovering = false;
	}

	function onMouseDown(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		dragging = true;
		addMargin = container.offsetWidth + $userSettings.trayWidth - e.clientX;
		document.addEventListener('mouseup', onMouseUp);
		document.addEventListener('mousemove', onMouseMove);
	}

	function onMouseEnter() {
		hovering = true;
	}

	function onMouseLeave() {
		if (!dragging) {
			hovering = false;
		}
	}

	function onMouseMove(e: MouseEvent) {
		userSettings.update((s) => ({
			...s,
			peekTrayWidth: e.clientX - $userSettings.trayWidth + addMargin
		}));
	}

	function onMouseUp() {
		dragging = false;
		document.removeEventListener('mouseup', onMouseUp);
		document.removeEventListener('mousemove', onMouseMove);
	}

	$: if (item == undefined || $item == undefined) expanded = false;
	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
</script>

<div
	bind:this={container}
	class:h-full={fullHeight}
	class:shadow-xl={expanded}
	style:top={fullHeight ? 0 : `${offsetTop}px`}
	style:width={`${$userSettings.peekTrayWidth}px`}
	style:translate={`${offsetLeft}px`}
	style:transition-property={!disabled ? (expanded ? 'top,translate,height' : 'translate') : 'none'}
	class="absolute z-30 flex shrink-0 overflow-y-auto overflow-x-visible bg-white text-light-800 duration-200 ease-in-out dark:bg-dark-800 dark:text-dark-100"
>
	<div class="w-full flex-grow overflow-y-scroll" style:width={`${$userSettings.peekTrayWidth}px`}>
		{#if $item instanceof BranchData}
			<RemoteBranchPeek {branchController} {base} branch={$item} />
		{:else if $item instanceof Branch}
			<VirtualBranchPeek {branchController} {base} branch={$item} />
		{:else if $item instanceof BaseBranch}
			<BaseBranchPeek base={$item} {branchController} />
		{:else}
			Unknown instance
		{/if}
	</div>
	<div
		class="flex w-4 cursor-pointer items-center bg-light-50 text-light-600 dark:bg-dark-700"
		role="button"
		tabindex="0"
		on:click={toggleExpanded}
		on:keypress={toggleExpanded}
		on:mouseenter={onMouseEnter}
		on:mouseleave={onMouseLeave}
	>
		<IconChevronLeft />
	</div>
	<div
		on:mousedown={onMouseDown}
		on:mouseenter={onMouseEnter}
		on:mouseleave={onMouseLeave}
		tabindex="0"
		role="slider"
		aria-valuenow={$userSettings.peekTrayWidth}
		class:bg-orange-300={hovering}
		class:dark:bg-orange-700={hovering}
		class="right-0 h-full w-0.5 shrink-0 cursor-ew-resize bg-light-50 text-light-600 dark:bg-dark-700"
	/>
</div>
