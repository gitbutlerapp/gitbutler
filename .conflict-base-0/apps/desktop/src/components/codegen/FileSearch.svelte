<script lang="ts">
	import { FileListItem, Icon } from '@gitbutler/ui';
	import { clickOutside } from '@gitbutler/ui/utils/clickOutside';
	import { fly } from 'svelte/transition';

	type Props = {
		files: string[] | undefined;
		indexOfSelectedFile: number | undefined;
		loading: boolean;
		onselect: (filename: string) => void;
		onexit: () => void;
	};

	const { onselect, files, loading, onexit, indexOfSelectedFile }: Props = $props();
</script>

{#if loading}
	<div
		class="file-plugin"
		use:clickOutside={{ handler: () => onexit() }}
		in:fly={{ y: 8, duration: 200 }}
	>
		<div class="p-12">
			<Icon name="spinner" />
		</div>
	</div>
{:else if files}
	<div
		class="file-plugin"
		use:clickOutside={{ handler: () => onexit() }}
		in:fly={{ y: 8, duration: 200 }}
	>
		{#each files as file, i}
			<FileListItem
				filePath={file}
				onclick={() => onselect(file)}
				selected={i === indexOfSelectedFile}
				active={i === indexOfSelectedFile}
			/>
		{:else}
			<!-- EMPTY STATE, SHOULD BE DESIGN VETTED -->
			<div class="p-12 text-12">No files found</div>
		{/each}
	</div>
{/if}

<style lang="postcss">
	.file-plugin {
		z-index: var(--z-floating);
		position: absolute;
		right: 12px;
		bottom: calc(100% + 6px);
		left: 12px;
		overflow-x: hidden;
		overflow-y: auto;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		background-color: var(--clr-bg-1);
		box-shadow: 0 10px 30px 0 color(srgb 0 0 0 / 0.16);
	}

	:global(.dark) .file-plugin {
		box-shadow: 0 10px 50px 5px color(srgb 0 0 0 / 0.5);
	}
</style>
