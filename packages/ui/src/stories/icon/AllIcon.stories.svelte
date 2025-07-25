<script module lang="ts">
	import Icon from '$components/Icon.svelte';
	import iconsJson from '$lib/data/icons.json';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / Icon'
	});

	let icons = Object.keys(iconsJson) as (keyof typeof iconsJson)[];
</script>

<Story name="All">
	<div class="icons">
		{#each icons as icon}
			<div class="icon-item">
				<Icon name={icon} />
				<span class="text-11 icon-label">
					{icon}
				</span>
			</div>
		{/each}
	</div>
</Story>

<style>
	.icons {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
		padding: 32px;
		gap: 16px;
	}

	.icon-item {
		display: flex;
		flex-direction: column;
		gap: 12px;
		color: var(--clr-text-1);
	}

	.icon-label {
		color: var(--clr-text-2);
	}
</style>
