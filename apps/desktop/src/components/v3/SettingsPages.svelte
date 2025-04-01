<script module>
	import iconsJson from '@gitbutler/ui/data/icons.json';
	import type { Component } from 'svelte';

	export type Page = {
		id: string;
		icon?: keyof typeof iconsJson;
		label: string;
		component: Component;
	};
</script>

<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import { newProjectSettingsPath } from '$lib/routes/routes.svelte';
	import Icon from '@gitbutler/ui/Icon.svelte';

	type Props = {
		projectId: string;
		title: string;
		selectedId?: string;
		pages: Page[];
	};

	const { projectId, title, selectedId: selectedId, pages }: Props = $props();

	const shownId = $derived(selectedId || pages[0]!.id);
	const shownPage = $derived(selectedId ? pages.find((p) => p.id === shownId) : pages[0]);
</script>

<div class="settings">
	<div class="pages">
		<div class="title">
			{title}
		</div>
		<div class="links">
			{#each pages as page}
				{@const selected = page.id === shownId}
				<a class="page-link" class:selected href={newProjectSettingsPath(projectId, page.id)}>
					{#if page.icon}
						<Icon name={page.icon} />
					{/if}
					{page.label}
				</a>
			{/each}
		</div>
	</div>
	<div class="page">
		<ConfigurableScrollableContainer>
			{#if shownPage}
				<shownPage.component />
			{:else}
				Settings page {selectedId} not Found.
			{/if}
		</ConfigurableScrollableContainer>
	</div>
</div>

<style lang="postcss">
	.settings {
		display: flex;
	}
	.pages {
		background-color: var(--clr-bg-1);
		display: flex;
		gap: 20px;
		flex-direction: column;
		padding: 12px 20px;
		border-radius: var(--radius-m) 0 0 var(--radius-m);
	}
	.page {
		display: flex;
		flex-direction: column;
	}
	.page-link {
		display: block;
	}
	.selected {
		background-color: var(--clr-bg-1-muted);
	}
	.title {
	}
</style>
