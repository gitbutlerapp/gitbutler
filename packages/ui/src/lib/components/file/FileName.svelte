<script lang="ts">
	import Tooltip from '$components/Tooltip.svelte';
	import FileIcon from '$components/file/FileIcon.svelte';
	import { splitFilePath } from '$lib/utils/filePath';

	interface Props {
		filePath: string;
		hideFilePath?: boolean;
		textSize?: '12' | '13';
		fileType?: 'regular' | 'executable' | 'symlink' | 'submodule';
		/**
		 * @deprecated Use fileType prop instead
		 */
		executable?: boolean;
	}

	let { filePath, textSize = '12', hideFilePath, fileType, executable }: Props = $props();
	const fileNameAndPath = $derived(splitFilePath(filePath));
	const filePathParts = $derived({
		first: fileNameAndPath.path.split('/').slice(0, -1).join('/'),
		last: fileNameAndPath.path.split('/').slice(-1).join('/')
	});
</script>

<div class="file-name">
	<FileIcon fileName={fileNameAndPath.filename} size={16} {fileType} {executable} />
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
		flex-shrink: 1;
		align-items: center;
		width: 100%;
		min-width: 32px;
		overflow: hidden;
		gap: 6px;
	}

	.file-name__name {
		flex-grow: 0;
		flex-shrink: 1;
		min-width: 40px;
		color: var(--clt-text-1);
		pointer-events: none;
	}

	.file-name__path-container {
		display: flex;
		flex-grow: 1;
		flex-shrink: 0;
		flex-basis: 0px;
		justify-content: flex-start;
		min-width: 16px;
		overflow: hidden;
		text-align: left;
	}

	.file-name__path {
		display: flex;
		align-items: center;
		max-width: 100%;
		color: var(--clt-text-1);
		line-height: 120%;
		text-align: left;
		opacity: 0.3;
	}

	.file-name__path--first,
	.file-name__path--last {
		min-width: 2ch;
	}
	.file-name__path--first {
		flex: 1;
		direction: rtl;
	}
</style>
