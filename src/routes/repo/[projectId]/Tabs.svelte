<script lang="ts">
	import IconTriangleUp from '$lib/icons/IconTriangleUp.svelte';
	import IconTriangleDown from '$lib/icons/IconTriangleDown.svelte';
	import { type ComponentType, getContext } from 'svelte';
	import lscache from 'lscache';
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/userSettings';
	import { slide } from 'svelte/transition';
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import Resizer from '$lib/components/Resizer.svelte';

	interface Tab {
		name: string;
		displayName: string;
		component: ComponentType;
		props: any;
	}

	const userSettings = getContext<SettingsStore>(SETTINGS_CONTEXT);
	const treeHeightKey = 'treeHeight:';
	const activeTabKey = 'activeTab:';
	const expandedKey = 'expanded:';

	export let items: Tab[] = [];
	export let branchId: string;

	$: expanded = branchId ? Boolean(lscache.get(expandedKey + branchId)) : false;
	$: activeTabValue = lscache.get(activeTabKey + branchId) ?? items[0].name;
	$: treeHeight = lscache.get(treeHeightKey + branchId) || $userSettings.defaultTreeHeight;

	let thViewport: HTMLElement;
	let thContents: HTMLElement;

	function setTreeExpanded(value: boolean) {
		expanded = value;
		lscache.set(expandedKey + branchId, expanded, 7 * 1440);
	}

	function setActiveTab(value: string) {
		activeTabValue = value;
		lscache.set(activeTabKey + branchId, activeTabValue, 7 * 1440);
	}
</script>

<div class="border-b border-t border-light-300 bg-light-50 dark:border-dark-500 dark:bg-dark-800">
	<div
		class="flex w-full border-b border-light-200 text-light-700 dark:border-dark-500 dark:text-dark-200"
	>
		{#each items as item}
			<button
				class:text-light-800={activeTabValue == item.name}
				class:dark:text-white={activeTabValue == item.name}
				class="-mb-px rounded-none p-2 font-medium"
				on:click={() => {
					if (activeTabValue == item.name && expanded) {
						setTreeExpanded(false);
						setActiveTab('');
						return;
					}
					setTreeExpanded(true);
					setActiveTab(item.name);
				}}
			>
				{item.displayName}
			</button>
		{/each}
		<div class="flex-grow" />
		<button
			class="flex items-center gap-x-4 py-0 text-light-600"
			on:click|stopPropagation={() => {
				setTreeExpanded(!expanded);
			}}
		>
			<div class="pr-3">
				{#if expanded}
					<IconTriangleUp />
				{:else}
					<IconTriangleDown />
				{/if}
			</div>
		</button>
	</div>
	{#if expanded}
		<div class="relative">
			<div
				class="hide-native-scrollbar relative shrink-0 overflow-scroll overscroll-none bg-white dark:bg-dark-1000"
				transition:slide|local={{ duration: 250 }}
				style:height={`${treeHeight}px`}
				bind:this={thViewport}
			>
				<div bind:this={thContents} class="h-full">
					{#each items as item}
						{#if activeTabValue == item.name}
							<svelte:component this={item.component} {...item.props} />
						{/if}
					{/each}
				</div>
			</div>
			<Scrollbar viewport={thViewport} contents={thContents} width="0.4rem" />
		</div>
	{/if}
</div>
<Resizer
	minHeight={40}
	viewport={thViewport}
	direction="vertical"
	class="z-30"
	on:height={(e) => {
		treeHeight = e.detail;
		console.log(branchId);
		lscache.set(treeHeightKey + branchId, e.detail, 7 * 1440); // 7 day ttl
		userSettings.update((s) => ({ ...s, defaultTreeHeight: e.detail }));
	}}
/>
