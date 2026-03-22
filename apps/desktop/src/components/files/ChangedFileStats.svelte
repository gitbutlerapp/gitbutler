<script lang="ts">
	import FileListViewToggle from "$components/files/FileListViewToggle.svelte";
	import { Badge, LineStats } from "@gitbutler/ui";

	type Props = {
		title: string;
		mode: "list" | "tree";
		persistId: string;
		fileCount?: number;
		linesAdded?: number;
		linesRemoved?: number;
	};

	let {
		title,
		mode = $bindable(),
		persistId,
		fileCount,
		linesAdded,
		linesRemoved,
	}: Props = $props();
</script>

<div class="stats-left">
	<h4 class="text-14 text-semibold title">{title}</h4>
	{#if fileCount !== undefined}
		<Badge>{fileCount}</Badge>
	{/if}
	{#if linesAdded !== undefined || linesRemoved !== undefined}
		<LineStats linesAdded={linesAdded ?? 0} linesRemoved={linesRemoved ?? 0} />
	{/if}
</div>
<FileListViewToggle bind:mode {persistId} />

<style lang="postcss">
	.stats-left {
		display: flex;
		flex: 1;
		align-items: center;
		min-width: 0;
		gap: 8px;
	}

	.title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
