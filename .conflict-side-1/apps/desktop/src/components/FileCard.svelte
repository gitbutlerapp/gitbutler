<script lang="ts">
	import ScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import FileCardHeader from '$components/FileCardHeader.svelte';
	import FileDiff from '$components/FileDiff.svelte';
	import { parseFileSections } from '$lib/utils/fileSections';
	import type { AnyFile } from '$lib/files/file';

	interface Props {
		file: AnyFile;
		isUnapplied: boolean;
		selectable?: boolean;
		readonly?: boolean;
		isCard?: boolean;
		commitId?: string;
		onClose?: () => void;
	}

	const {
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
			pointer-events: none;
			content: '';
			position: absolute;
			top: 0;
			right: 50%;
			transform: translateX(50%);
			width: 1px;
			height: 100%;
		}
	}

	.file-card {
		background: var(--clr-bg-1);
		overflow: hidden;
		display: flex;
		flex-direction: column;
		max-height: 100%;
		flex-grow: 1;
	}
</style>
