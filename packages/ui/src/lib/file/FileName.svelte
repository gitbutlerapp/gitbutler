<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import FileIcon from '$lib/file/FileIcon.svelte';
	import { splitFilePath } from '$lib/utils/filePath';
	import type { FileStatus } from '$lib/file/types';

	interface Props {
		ref?: HTMLDivElement;
		filePath: string;
		fileStatus?: FileStatus;
		draggable?: boolean;
		textSize?: '12' | '13';
	}

	let { ref = $bindable(), filePath, textSize = '12' }: Props = $props();
	const fileNameAndPath = $derived(splitFilePath(filePath));
	const filePathParts = $derived({
		first: fileNameAndPath.path.split('/').slice(0, -1).join('/'),
		last: fileNameAndPath.path.split('/').slice(-1).join('/')
	});
</script>

<div role="presentation" bind:this={ref} class="file-header">
	<FileIcon fileName={fileNameAndPath.filename} size={16} />
	<span class="text-{textSize} text-semibold file-header__name truncate">
		{fileNameAndPath.filename}
	</span>

	{#if fileNameAndPath.path}
		<div class="file-header__path-container">
			<Tooltip text={filePath} delay={1200}>
				<p class="text-12 file-header__path truncate">
					{#if filePathParts.first}
						<span class="file-header__path--first truncate">
							{filePathParts.first}
						</span>
						/
					{/if}

					<span class="file-header__path--last truncate">
						{filePathParts.last}
					</span>
				</p>
			</Tooltip>
		</div>
	{/if}
</div>

<style lang="postcss">
	.file-header {
		display: flex;
		align-items: center;
		flex-shrink: 1;
		min-width: 32px;
		gap: 6px;
		width: 100%;
		overflow: hidden;
	}

	.file-header__name {
		flex-shrink: 1;
		flex-grow: 0;
		min-width: 40px;
		pointer-events: none;
		color: var(--clt-text-1);
	}

	.file-header__path-container {
		display: flex;
		justify-content: flex-start;
		flex-shrink: 0;
		flex-grow: 1;
		flex-basis: 0px;
		text-align: left;
		min-width: 16px;
		overflow: hidden;
	}

	.file-header__path {
		display: flex;
		align-items: center;
		color: var(--clt-text-1);
		line-height: 120%;
		opacity: 0.3;
		max-width: 100%;
		text-align: left;
	}

	.file-header__path--first,
	.file-header__path--last {
		direction: rtl;
		min-width: 2ch;
	}

	.file-header__path--first {
		flex: 1;
	}
</style>
