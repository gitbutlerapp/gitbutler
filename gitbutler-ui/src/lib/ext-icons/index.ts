/**
 * https://github.com/gitbutlerapp/file-extension-icon-JS
 */
import { vsiFileExtensionsToIcons, vsiFileNamesToIcons } from './vsi/typeMap';
import { vsiFileIcons1 } from './vsi/vsiFileIcons1';
import { vsiFileIcons10 } from './vsi/vsiFileIcons10';
import { vsiFileIcons11 } from './vsi/vsiFileIcons11';
import { vsiFileIcons12 } from './vsi/vsiFileIcons12';
import { vsiFileIcons13 } from './vsi/vsiFileIcons13';
import { vsiFileIcons14 } from './vsi/vsiFileIcons14';
import { vsiFileIcons2 } from './vsi/vsiFileIcons2';
import { vsiFileIcons3 } from './vsi/vsiFileIcons3';
import { vsiFileIcons4 } from './vsi/vsiFileIcons4';
import { vsiFileIcons5 } from './vsi/vsiFileIcons5';
import { vsiFileIcons6 } from './vsi/vsiFileIcons6';
import { vsiFileIcons7 } from './vsi/vsiFileIcons7';
import { vsiFileIcons8 } from './vsi/vsiFileIcons8';
import { vsiFileIcons9 } from './vsi/vsiFileIcons9';

export const vsiFileIcons: { [key: string]: string } = {
	...vsiFileIcons1,
	...vsiFileIcons2,
	...vsiFileIcons3,
	...vsiFileIcons4,
	...vsiFileIcons5,
	...vsiFileIcons6,
	...vsiFileIcons7,
	...vsiFileIcons8,
	...vsiFileIcons9,
	...vsiFileIcons10,
	...vsiFileIcons11,
	...vsiFileIcons12,
	...vsiFileIcons13,
	...vsiFileIcons14
};

function convertToBase64(iconString: string) {
	try {
		return btoa(iconString);
	} catch (err) {
		return Buffer.from(iconString).toString('base64');
	}
}

export function getVSIFileIcon(fileName: string) {
	fileName = fileName.toLowerCase();
	const splitName = fileName.split('.');
	let iconName = '';

	while (splitName.length) {
		const curName = splitName.join('.');
		if (vsiFileNamesToIcons[curName]) {
			iconName = vsiFileNamesToIcons[curName];
			break;
		}
		if (vsiFileExtensionsToIcons[curName]) {
			iconName = vsiFileExtensionsToIcons[curName];
			break;
		}

		splitName.shift();
	}

	if (iconName === '') iconName = 'file';
	const icon = vsiFileIcons[iconName];
	return `data:image/svg+xml;base64,${convertToBase64(icon)}`;
}
