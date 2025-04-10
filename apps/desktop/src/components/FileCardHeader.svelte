<script lang="ts">
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { computeAddedRemovedByFiles } from '$lib/utils/metrics';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import FileName from '@gitbutler/ui/file/FileName.svelte';
	import FileStatusBadge from '@gitbutler/ui/file/FileStatusBadge.svelte';
	import LineChangeStats from '@gitbutler/ui/file/LineChangeStats.svelte';
	import type { AnyFile } from '$lib/files/file';

	interface Props {
		file: AnyFile;
		isFileLocked: boolean;
		onClose?: () => void;
	}

	const { file, isFileLocked, onClose }: Props = $props();

	const fileStats = $derived(computeAddedRemovedByFiles(file));
	const fileStatus = $derived(computeFileStatus(file));
</script>

<div class="header">
	<div class="header__inner">
		<div class="header__info truncate">
			<FileName filePath={file.path} textSize="13" />
			<div class="header__tags">
				<LineChangeStats added={fileStats.added} removed={fileStats.removed} />

				{#if fileStatus}
					<FileStatusBadge status={fileStatus} style="full" />
				{/if}

				{#if file.conflicted || isFileLocked}
					<div class="header__tag-group">
						{#if isFileLocked}
							<Badge
								size="icon"
								icon="locked-small"
								style="warning"
								tooltip="File changes cannot be moved because part of this file was already committed into this branch"
								>Locked</Badge
							>
						{/if}
						{#if file.conflicted}
							<Badge size="icon" style="error">Has conflicts</Badge>
						{/if}
					</div>
				{/if}
			</div>
		</div>
	</div>
	<Button icon="cross" kind="ghost" onclick={() => onClose?.()} />
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
		gap: 8px;
	}
	.header__tag-group {
		display: flex;
		gap: 4px;
	}
</style>
