<script module lang="ts">
	import FileViewHeader from '$components/file/FileViewHeader.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const { Story } = defineMeta({
		title: 'List items / FileViewHeader',
		component: FileViewHeader
	});
</script>

<script lang="ts">
</script>

<Story name="Sticky test">
	{#snippet template()}
		<div class="scroll-wrap">
			{#each Array(20) as _}
				<div class="card-example">
					<FileViewHeader
						filePath="src/component/arcada/path/binary/file.svelte"
						fileStatus="M"
						draggable={true}
						linesAdded={10}
						linesRemoved={5}
					/>
					<p class="content-example">some content</p>
				</div>
			{/each}
		</div>
	{/snippet}
</Story>

<style>
	.scroll-wrap {
		display: flex;
		position: relative;
		flex-direction: column;
		width: 100%;
		max-width: 400px;
		height: 300px;
		overflow-x: hidden;
		overflow-y: auto;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}

	.card-example {
		display: flex;
		flex-direction: column;
		width: 100%;
	}

	.content-example {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 200px;
		background-color: rgba(0, 0, 0, 0.05);
	}
</style>
