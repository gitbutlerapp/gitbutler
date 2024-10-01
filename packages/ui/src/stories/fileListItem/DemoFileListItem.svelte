<script lang="ts">
	import { type FileStatus } from '$lib/file/types';
	import FileListItem from '$lib/file/FileListItem.svelte';

	interface Props {
		filePath: string;
		fileStatus?: FileStatus;
		draggable?: boolean;
		fileStatusStyle?: 'dot' | 'full';
		clickable?: boolean;
		selected?: boolean;
		showCheckbox?: boolean;
		checked?: boolean;
		conflicted?: boolean;
		lockText?: string;
		oncheck?: (e: Event) => void;
		onclick?: () => void;
		onkeydown?: () => void;
		ondragstart?: (e: DragEvent) => void;
		oncontextmenu?: (e: MouseEvent) => void;
	}

	let {
		filePath,
		fileStatus,
		draggable = false,
		fileStatusStyle = 'dot',
		clickable = true,
		selected = false,
		showCheckbox = false,
		checked,
		conflicted,
		lockText,
		oncheck,
		onclick,
		onkeydown,
		ondragstart,
		oncontextmenu
	}: Props = $props();

	let ref: HTMLDivElement | undefined = $state();

	$effect(() => {
		if (ref) {
			console.log('FileListItem updated', ref);
		}
	});
</script>

<FileListItem
	bind:ref
	{filePath}
	{fileStatus}
	{fileStatusStyle}
	{draggable}
	{clickable}
	{selected}
	{showCheckbox}
	{checked}
	{conflicted}
	{lockText}
	{oncheck}
	{onclick}
	{onkeydown}
	{ondragstart}
	{oncontextmenu}
/>
