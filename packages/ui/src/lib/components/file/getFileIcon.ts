import { fileIcons } from '$components/file/fileIcons';
import { symbolFileExtensionsToIcons, symbolFileNamesToIcons } from '$components/file/typeMap';
import { convertToBase64 } from '$lib/utils/convertToBase64';

export function getFileIcon(fileName: string) {
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
	return `data:image/svg+xml;base64,${convertToBase64(icon)}`;
}
