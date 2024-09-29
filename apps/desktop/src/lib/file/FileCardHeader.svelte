<script lang="ts">
	import FileStatusTag from './FileStatusTag.svelte';
	import { splitFilePath } from '$lib/utils/filePath';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { computeAddedRemovedByFiles } from '$lib/utils/metrics';
	import Button from '@gitbutler/ui/Button.svelte';
	import FileIcon from '@gitbutler/ui/file/FileIcon.svelte';
	import { createEventDispatcher } from 'svelte';
	import type { AnyFile } from '$lib/vbranches/types';

	interface Props {
		file: AnyFile;
		isFileLocked: boolean;
	}

	let { file, isFileLocked }: Props = $props();

	const dispatch = createEventDispatcher<{ close: void }>();
	let fileStats = $derived(computeAddedRemovedByFiles(file));
	let fileStatus = $derived(computeFileStatus(file));

	let fileTitle = $derived(splitFilePath(file.path));
</script>

<div class="header">
	<div class="header__inner">
		<FileIcon fileName={file.path} size={16} />
		<div class="header__info truncate">
			<div class="header__filetitle text-13 truncate">
				<span class="header__filename">{fileTitle.filename}</span>
				<span class="header__filepath">{fileTitle.path}</span>
			</div>
			<div class="header__tags">
				{#if file.conflicted || isFileLocked}
					<div class="header__tag-group">
						{#if isFileLocked}
							<Button
								size="tag"
								clickable={false}
								icon="locked-small"
								style="warning"
								tooltip="File changes cannot be moved because part of this file was already committed into this branch"
								>Locked</Button
							>
						{/if}
						{#if file.conflicted}
							<Button size="tag" clickable={false} icon="warning-small" style="error"
								>Has conflicts</Button
							>
						{/if}
					</div>
				{/if}
				<div class="header__tag-group">
					{#if fileStats.added}
						<Button size="tag" clickable={false} style="success">+{fileStats.added}</Button>
					{/if}
					{#if fileStats.removed}
						<Button size="tag" clickable={false} style="error">-{fileStats.removed}</Button>
					{/if}
					{#if fileStatus}
						<FileStatusTag status={fileStatus} />
					{/if}
				</div>
			</div>
		</div>
	</div>
	<Button icon="cross" style="ghost" onclick={() => dispatch('close')} />
</div>

<style lang="postcss">
	.header {
		display: flex;
		padding: 10px;
		gap: 12px;
		border-bottom: 1px solid var(--clr-border-2);
	}
	.header__inner {
		display: flex;
		flex-grow: 1;
		gap: 8px;
		padding: 4px;
		overflow: hidden;
	}
	.header__info {
		display: flex;
		flex-direction: column;
		gap: 8px;
		width: 100%;
	}
	.header__tags {
		display: flex;
		gap: 6px;
	}
	.header__tag-group {
		display: flex;
		gap: 2px;
	}
	.header__filetitle {
		width: 100%;
		user-select: text;
	}
	.header__filename {
		color: var(--clr-scale-ntrl-0);
		line-height: 120%;
	}
	.header__filepath {
		color: var(--clr-scale-ntrl-50);
	}
</style>
