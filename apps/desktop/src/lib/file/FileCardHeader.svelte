<script lang="ts">
	import FileStatusTag from './FileStatusTag.svelte';
	import { getFileIcon } from '$lib/ext-icons';
	import { splitFilePath } from '$lib/utils/filePath';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { computeAddedRemovedByFiles } from '$lib/utils/metrics';
	import Button from '@gitbutler/ui/inputs/Button.svelte';
	import { createEventDispatcher } from 'svelte';
	import type { AnyFile } from '$lib/vbranches/types';

	export let file: AnyFile;
	export let isFileLocked: boolean;

	const dispatch = createEventDispatcher<{ close: void }>();
	$: fileStats = computeAddedRemovedByFiles(file);
	$: fileStatus = computeFileStatus(file);

	$: fileTitle = splitFilePath(file.path);
</script>

<div class="header">
	<div class="header__inner">
		<img src={getFileIcon(file.path)} alt="" class="icon" />
		<div class="header__info truncate">
			<div class="header__filetitle text-base-13 truncate">
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
								help="File changes cannot be moved because part of this file was already committed into this branch"
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
	<Button icon="cross" style="ghost" on:click={() => dispatch('close')} />
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
	.icon {
		flex-shrink: 0;
		width: 14px;
		height: 14px;
	}
</style>
