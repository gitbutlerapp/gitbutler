<script lang="ts">
	import FileListItem from '$lib/file/FileListItem.svelte';
	import { type FileStatus } from '$lib/file/types';

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
		onresolveclick?: () => void;
		onkeydown?: () => void;
		ondragstart?: (e: DragEvent) => void;
		oncontextmenu?: (e: MouseEvent) => void;
	}

	const {
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
		onresolveclick,
		onkeydown,
		ondragstart,
		oncontextmenu
	}: Props = $props();

	let ref: HTMLDivElement | undefined = $state();
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
	{onresolveclick}
	{onkeydown}
	{ondragstart}
	{oncontextmenu}
/>
