/**
 * https://github.com/wilfriedago/vscode-symbols-icon-theme
 */
import { fileIcons } from './symbol/fileIcons';
import { symbolFileExtensionsToIcons, symbolFileNamesToIcons } from './symbol/typeMap';

function convertToBase64(iconString: string) {
	try {
		return btoa(iconString);
	} catch (err) {
		return Buffer.from(iconString).toString('base64');
	}
}

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
