<script lang="ts">
	import { fileIcons } from './data/file/fileIcons';
	import { symbolFileExtensionsToIcons, symbolFileNamesToIcons } from './data/file/typeMap';
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		fileName: string;
		size: number;
	}

	const { fileName, size = 16 }: Props = $props();

	function getFileIcon(fileName: string) {
		fileName = fileName.toLowerCase();
		const splitName = fileName.split('.');
		let iconName = '';

		while (splitName.length) {
			const curName = splitName.join('.');
			if (symbolFileNamesToIcons[curName]) {
				iconName = symbolFileNamesToIcons[curName] ?? '';
				break;
			}
			if (symbolFileExtensionsToIcons[curName]) {
				iconName = symbolFileExtensionsToIcons[curName] ?? '';
				break;
			}

			splitName.shift();
		}

		if (iconName === '') {
			iconName = 'document';
		}
		let icon = fileIcons[iconName];
		if (!icon) {
			icon = fileIcons['document'] as string;
		}
		return icon;
	}
</script>

<div class="file-icon" style:--file-icon-size={pxToRem(size)}>
	{@html getFileIcon(fileName)}
</div>

<style lang="postcss">
	.file-icon {
		width: var(--file-icon-size);
		height: var(--file-icon-size);
	}
</style>
