<script lang="ts">
	import { FileListItem, Icon } from '@gitbutler/ui';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { fly } from 'svelte/transition';

	type Props = {
		files: string[] | undefined;
		indexOfSelectedFile: number | undefined;
		loading: boolean;
		query: string;
		onselect: (filename: string) => void;
		onexit: () => void;
	};

	const { onselect, files, loading, onexit, indexOfSelectedFile, query }: Props = $props();

	const shouldShow = $derived(files !== undefined);
</script>

{#if shouldShow}
	<div
		class="dialog-popup"
		use:clickOutside={{ handler: () => onexit() }}
		in:fly={{ y: 8, duration: 200 }}
	>
		<div class="dialog-popup__header">
			<Icon name="search-small" color="var(--clr-text-3)" />
			<h3 class="text-12 text-bold flex-1">Search for files</h3>
			<p class="text-11 clr-text-3">
				<span class="text-italic text-bold"> Esc </span>
				to close
			</p>
		</div>

		{#if loading}
			<div class="text-12 text-italic dialog-popup__placeholder-content">Loading...</div>
		{:else if files && files.length === 0 && query.length === 0}
			<div class="text-12 text-italic dialog-popup__placeholder-content">
				Type something to start searching…
			</div>
		{:else if files && files.length === 0}
			<div class="text-12 dialog-popup__placeholder-content">No files found ¯\_(ツ)_/¯</div>
		{:else if files}
			<div class="dialog-popup__content">
				{#each files as file, i}
					<FileListItem
						filePath={file}
						onclick={() => onselect(file)}
						selected={i === indexOfSelectedFile}
						active={i === indexOfSelectedFile}
					/>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<style lang="postcss">
	.dialog-popup {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;
		right: 12px;
		bottom: calc(100% + 6px);
		left: 12px;
		flex-direction: column;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
		box-shadow: 0 10px 30px 0 color(srgb 0 0 0 / 0.16);
	}

	:global(.dark) .dialog-popup {
		box-shadow: 0 10px 50px 5px color(srgb 0 0 0 / 0.5);
	}

	.dialog-popup__header {
		display: flex;
		align-items: center;
		width: 100%;
		padding: 10px 10px;
		gap: 6px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.dialog-popup__content {
		max-height: 300px;
		overflow-x: hidden;
		overflow-y: auto;
	}

	.dialog-popup__placeholder-content {
		padding: 14px 14px;
		background-color: var(--clr-bg-2);
		color: var(--clr-text-3);
		text-align: center;
	}
</style>
