<script module lang="ts">
	import { fileIcons } from '$lib/components/file/fileIcons.js';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'Basic / FileIcon'
	});

	let fileIconEntries = Object.entries(fileIcons);
</script>

<Story name="All">
	<div class="icons">
		{#each fileIconEntries as [iconName, iconSvg]}
			<div class="icon-item">
				<div class="icon-preview">
					{@html iconSvg}
				</div>
				<span class="text-11 icon-label">
					{iconName}
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

	.icon-preview {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 16px;
		height: 16px;
	}

	.icon-label {
		color: var(--clr-text-2);
	}
</style>
