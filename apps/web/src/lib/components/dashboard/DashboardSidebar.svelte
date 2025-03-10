<script lang="ts">
	import DashboardSidebarProjects from '$lib/components/dashboard/DashboardSidebarProjects.svelte';
	import { dashboardSidebarSetTab, type SidebarTab } from '$lib/dashboard/sidebar.svelte';
	import { WebState } from '$lib/redux/store.svelte';
	import { getContext } from '@gitbutler/shared/context';

	const webState = getContext(WebState);
	const webDispatch = webState.appDispatch;

	const currentTab = $derived(webState.dashboardSidebar.currentTab);

	const tabs = [
		{ label: 'My projects', key: 'projects' as SidebarTab },
		{ label: 'My reviews', key: 'reviews' as SidebarTab }
	];
</script>

<div class="sidebar">
	<div class="tabs">
		{#each tabs as tab}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<!-- svelte-ignore a11y_click_events_have_key_events -->
			<div
				class="text-13 text-bold tab"
				class:current={currentTab === tab.key}
				onclick={() => {
					webDispatch.dispatch(dashboardSidebarSetTab(tab.key));
				}}
			>
				{tab.label}
			</div>
		{/each}
	</div>
	<div class="content">
		{#if currentTab === 'projects'}
			<DashboardSidebarProjects />
		{/if}
	</div>
</div>

<style lang="postcss">
	.sidebar {
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);

		background-color: var(--clr-bg-1);

		overflow: hidden;
	}

	.tabs {
		display: flex;
		width: 100%;
	}

	.tab {
		text-align: center;
		flex-grow: 1;
		background-color: var(--clr-bg-2);
		border: 1px solid var(--clr-border-2);
		border-top: none;
		border-right: none;

		padding: 16px 0;

		&:first-child {
			border-left: none;
		}

		&.current {
			background-color: var(--clr-bg-1);
			border-bottom: none;
		}
	}

	.content {
		max-height: calc(75vh - 100px);
		min-height: 24px;

		overflow-x: auto;
	}
</style>
