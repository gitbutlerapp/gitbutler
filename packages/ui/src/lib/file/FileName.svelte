<script lang="ts">
	import Tooltip from '$lib/Tooltip.svelte';
	import FileIcon from '$lib/file/FileIcon.svelte';
	import { splitFilePath } from '$lib/utils/filePath';

	interface Props {
		filePath: string;
		hideFilePath?: boolean;
		textSize?: '12' | '13';
	}

	let { filePath, textSize = '12', hideFilePath }: Props = $props();
	const fileNameAndPath = $derived(splitFilePath(filePath));
	const filePathParts = $derived({
		first: fileNameAndPath.path.split('/').slice(0, -1).join('/'),
		last: fileNameAndPath.path.split('/').slice(-1).join('/')
	});
</script>

<div role="presentation" class="file-name">
	<FileIcon fileName={fileNameAndPath.filename} size={16} />
	<span class="text-{textSize} text-semibold file-name__name truncate">
		{fileNameAndPath.filename}
	</span>

	{#if fileNameAndPath.path && !hideFilePath}
		<div class="file-name__path-container">
			<Tooltip text={filePath} delay={1200} maxWidth={320}>
				<p class="text-12 file-name__path truncate">
					{#if filePathParts.first}
						<span class="file-name__path--first truncate">
							{filePathParts.first}
						</span>
						/
					{/if}

					<span class="file-name__path--last truncate">
						{filePathParts.last}
					</span>
				</p>
			</Tooltip>
		</div>
	{/if}
</div>

<style lang="postcss">
	.file-name {
		display: flex;
		align-items: center;
		flex-shrink: 1;
		min-width: 32px;
		gap: 6px;
		width: 100%;
		overflow: hidden;
	}

	.file-name__name {
		flex-shrink: 1;
		flex-grow: 0;
		min-width: 40px;
		pointer-events: none;
		color: var(--clt-text-1);
	}

	.file-name__path-container {
		display: flex;
		justify-content: flex-start;
		flex-shrink: 0;
		flex-grow: 1;
		flex-basis: 0px;
		text-align: left;
		min-width: 16px;
		overflow: hidden;
	}

	.file-name__path {
		display: flex;
		align-items: center;
		color: var(--clt-text-1);
		line-height: 120%;
		opacity: 0.3;
		max-width: 100%;
		text-align: left;
	}

	.file-name__path--first,
	.file-name__path--last {
		direction: rtl;
		min-width: 2ch;
	}

	.file-name__path--first {
		flex: 1;
	}
</style>
