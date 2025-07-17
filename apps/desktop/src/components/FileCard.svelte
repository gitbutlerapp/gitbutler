<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileCardHeader from '$components/FileCardHeader.svelte';
	import FileDiff from '$components/FileDiff.svelte';
	import { parseFileSections } from '$lib/utils/fileSections';
	import type { AnyFile } from '$lib/files/file';

	interface Props {
		projectId: string;
		file: AnyFile;
		isUnapplied: boolean;
		selectable?: boolean;
		readonly?: boolean;
		isCard?: boolean;
		commitId?: string;
		onClose?: () => void;
	}

	const {
		projectId,
		file,
		isUnapplied,
		commitId,
		selectable = false,
		readonly = false,
		isCard = true,
		onClose
	}: Props = $props();

	const sections = $derived.by(() => {
		if (file.binary || file.large) {
			return [];
		}

		return parseFileSections(file);
	});
</script>

<div id={`file-${file.id}`} class="file-card" class:card={isCard}>
	<FileCardHeader {file} isFileLocked={file.locked} {onClose} />

	<ScrollableContainer wide>
		<FileDiff
			{projectId}
			filePath={file.path}
			isLarge={file.large}
			isBinary={file.binary}
			{readonly}
			{sections}
			isFileLocked={file.locked}
			{isUnapplied}
			{selectable}
			{commitId}
		/>
	</ScrollableContainer>
</div>

<div class="divider-line"></div>

<style lang="postcss">
	.divider-line {
		position: absolute;
		top: 0;
		right: 0;
		width: 1px;
		height: 100%;

		&:after {
			position: absolute;
			top: 0;
			right: 50%;
			width: 1px;
			height: 100%;
			transform: translateX(50%);
			content: '';
			pointer-events: none;
		}
	}

	.file-card {
		display: flex;
		flex-grow: 1;
		flex-direction: column;
		max-height: 100%;
		overflow: hidden;
		background: var(--clr-bg-1);
	}
</style>
