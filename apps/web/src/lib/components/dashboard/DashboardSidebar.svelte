<script lang="ts">
	import DashboardSidebarProjects from '$lib/components/dashboard/DashboardSidebarProjects.svelte';
	import DashboardSidebarReviews from '$lib/components/dashboard/DashboardSidebarReviews.svelte';
	import { dashboardSidebarSetTab, type SidebarTab } from '$lib/dashboard/sidebar.svelte';
	import { WEB_STATE } from '$lib/redux/store.svelte';
	import { inject } from '@gitbutler/shared/context';

	const webState = inject(WEB_STATE);
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
			<button
				type="button"
				aria-label="tab"
				class="text-13 text-bold tab"
				class:current={currentTab === tab.key}
				onclick={() => {
					webDispatch.dispatch(dashboardSidebarSetTab(tab.key));
				}}
			>
				{tab.label}
			</button>
		{/each}
	</div>
	<div class="content">
		{#if currentTab === 'projects'}
			<DashboardSidebarProjects />
		{:else if currentTab === 'reviews'}
			<DashboardSidebarReviews />
		{/if}
	</div>
</div>

<style lang="postcss">
	.sidebar {
		align-self: flex-start;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-l);
		background-color: var(--clr-bg-1);
	}

	.tabs {
		display: flex;
		width: 100%;
	}

	.tab {
		flex-grow: 1;

		padding: 16px 0;
		border: 1px solid var(--clr-border-2);
		border-top: none;
		border-right: none;
		background-color: var(--clr-bg-2);
		text-align: center;
		cursor: pointer;

		&:first-child {
			border-left: none;
		}

		&.current {
			border-bottom: none;
			background-color: var(--clr-bg-1);
		}
	}

	.content {
		min-height: 24px;
		max-height: calc(75vh - 100px);

		overflow-x: auto;
	}
</style>
