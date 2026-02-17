<script lang="ts">
	import Tooltip from "$components/Tooltip.svelte";
	import FileIcon from "$components/file/FileIcon.svelte";
	import { splitFilePath } from "$lib/utils/filePath";

	interface Props {
		filePath: string;
		hideFilePath?: boolean;
		textSize?: "12" | "13";
		pathFirst?: boolean;
		color?: string;
	}

	let { filePath, textSize = "12", hideFilePath, pathFirst = true, color }: Props = $props();
	const fileNameAndPath = $derived(splitFilePath(filePath));
	const filePathParts = $derived({
		first: fileNameAndPath.path.split("/").slice(0, -1).join("/"),
		last: fileNameAndPath.path.split("/").slice(-1).join("/"),
	});
</script>

<div class="file-name" style:--filename-color={color}>
	<div class="file-name__icon-container">
		<FileIcon fileName={fileNameAndPath.filename} {color} />
	</div>

	{#if pathFirst}
		{#if fileNameAndPath.path && !hideFilePath}
			<Tooltip text={filePath} delay={1200} maxWidth={320}>
				<span class="text-12 file-name__path-container file-name__path--first truncate">
					{fileNameAndPath.path}/
				</span>
			</Tooltip>
		{/if}
		<span class="text-{textSize} text-semibold file-name__name truncate">
			{fileNameAndPath.filename}
		</span>
	{:else}
		<span class="text-{textSize} text-semibold file-name__name truncate">
			{fileNameAndPath.filename}
		</span>

		{#if fileNameAndPath.path && !hideFilePath}
			<Tooltip text={filePath} delay={1200} maxWidth={320}>
				<span class="text-12 file-name__path-container file-name__path--last truncate">
					{#if filePathParts.first}
						<span class="file-name__path-first-part truncate">
							{filePathParts.first}
						</span>
						/
					{/if}
					<span class="file-name__path-last-part truncate">
						{filePathParts.last}
					</span>
				</span>
			</Tooltip>
		{/if}
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
		color: var(--filename-color, var(--clr-text-1));
	}

	.file-name__icon-container {
		display: flex;
		flex-shrink: 0;
		margin-right: 8px;
	}

	.file-name__path-container {
		flex: 1;
		min-width: 2ch;
		overflow: hidden;
		line-height: 120%;
		text-align: left;
	}

	.file-name__path--first {
		max-width: max-content;
		margin-right: 3px;
		opacity: 0.6;
	}

	.file-name__path--last {
		display: flex;
		align-items: center;
		max-width: max-content;
		margin-left: 6px;
		text-align: left;
		opacity: 0.4;
	}

	.file-name__path-first-part {
		flex: 1;
	}

	.file-name__name {
		overflow: hidden;
		white-space: nowrap;
		pointer-events: none;
	}
</style>
