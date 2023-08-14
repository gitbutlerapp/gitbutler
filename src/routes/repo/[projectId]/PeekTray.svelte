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
	import Resizer from '$lib/components/Resizer.svelte';

	export let item: Readable<BranchData | Branch | BaseBranch | undefined> | undefined;
	export let base: BaseBranch | undefined;
	export let branchController: BranchController;
	export let expanded: boolean;
	export let offsetTop: number;
	export let fullHeight = false;
	export let disabled = false;

	let viewport: HTMLElement;

	$: offsetLeft = expanded
		? $userSettings.trayWidth
		: $userSettings.trayWidth - $userSettings.peekTrayWidth;

	export function close() {
		expanded = false;
	}

	function toggleExpanded() {
		expanded = !expanded;
	}

	$: if (item == undefined || $item == undefined) expanded = false;
	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
</script>

<div
	class:h-full={fullHeight}
	class:shadow-xl={expanded}
	style:top={fullHeight ? 0 : `${offsetTop}px`}
	style:width={`${$userSettings.peekTrayWidth}px`}
	style:translate={`${offsetLeft}px`}
	style:transition-property={!disabled ? (expanded ? 'top,translate,height' : 'translate') : 'none'}
	class="absolute z-30 flex shrink-0 overflow-visible bg-white text-light-800 duration-200 ease-in-out dark:bg-dark-800 dark:text-dark-100"
>
	<div class="flex w-full flex-grow" bind:this={viewport}>
		<div
			class="w-full flex-grow overflow-y-scroll"
			style:width={`${$userSettings.peekTrayWidth}px`}
		>
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
		>
			<IconChevronLeft />
		</div>
		<Resizer
			{viewport}
			direction="horizontal"
			class="z-30"
			on:width={(e) => {
				userSettings.update((s) => ({
					...s,
					peekTrayWidth: e.detail
				}));
			}}
		/>
	</div>
</div>
