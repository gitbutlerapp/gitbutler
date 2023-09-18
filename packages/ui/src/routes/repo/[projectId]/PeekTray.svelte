<script lang="ts">
	import IconChevronLeft from '$lib/icons/IconChevronLeft.svelte';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { BaseBranch, Branch, RemoteBranch } from '$lib/vbranches/types';
	import { getContext } from 'svelte';
	import type { Readable } from '@square/svelte-store';
	import BaseBranchPeek from './BaseBranchPeek.svelte';
	import RemoteBranchPeek from './RemoteBranchPeek.svelte';
	import Resizer from '$lib/components/Resizer.svelte';
	import Lane from './BranchLane.svelte';
	import type { getCloudApiClient } from '$lib/api/cloud/api';

	export let item: Readable<RemoteBranch | Branch | BaseBranch | undefined> | undefined;
	export let cloud: ReturnType<typeof getCloudApiClient>;
	export let base: BaseBranch | undefined;
	export let branchController: BranchController;
	export let expanded: boolean;
	export let offsetTop: number;
	export let projectId: string;
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

	// Close by clicking on anything that bubbles up to the document. Note
	// that we need to stop propagation when clicking inside the peek tray,
	// and that we also want the same in the regular tray.
	function onDocumentClick() {
		expanded = false;
	}

	$: expanded
		? document.addEventListener('click', onDocumentClick)
		: document.removeEventListener('click', onDocumentClick);
</script>

<div
	class:h-full={fullHeight}
	class:shadow-xl={expanded}
	style:top={fullHeight ? 0 : `${offsetTop}px`}
	style:width={`${$userSettings.peekTrayWidth}px`}
	style:translate={`${offsetLeft}px`}
	style:transition-property={!disabled ? (expanded ? 'top,translate' : 'translate') : 'none'}
	class="bg-color-5 text-color-1 absolute z-30 flex shrink-0 overflow-visible outline-none duration-300 ease-in-out"
	on:click|stopPropagation
	on:keydown|stopPropagation
	role="menu"
	tabindex="0"
>
	<div class="flex w-full flex-grow" bind:this={viewport}>
		<div
			class="h-full max-h-full w-full flex-grow overflow-y-hidden"
			style:width={`${$userSettings.peekTrayWidth}px`}
		>
			{#if $item instanceof RemoteBranch}
				<RemoteBranchPeek {projectId} {branchController} branch={$item} />
			{:else if $item instanceof Branch}
				<Lane
					branch={$item}
					{branchController}
					{base}
					{cloud}
					{projectId}
					maximized={true}
					cloudEnabled={false}
					projectPath=""
					readonly={true}
				/>
			{:else if $item instanceof BaseBranch}
				<BaseBranchPeek {projectId} base={$item} {branchController} />
			{:else}
				Unknown instance
			{/if}
		</div>
		<div
			class="bg-color-4 hover:bg-color-5 text-color-4 flex w-4 cursor-pointer items-center"
			role="button"
			tabindex="0"
			on:click={toggleExpanded}
			on:keypress={toggleExpanded}
		>
			<IconChevronLeft />
		</div>
	</div>
	<Resizer
		{viewport}
		direction="horizontal"
		class="z-30"
		minWidth={200}
		on:width={(e) => {
			userSettings.update((s) => ({
				...s,
				peekTrayWidth: e.detail
			}));
		}}
	/>
</div>
