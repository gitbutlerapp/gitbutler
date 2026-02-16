import { fileIcons } from "$components/file/fileIcons";
import { symbolFileExtensionsToIcons, symbolFileNamesToIcons } from "$components/file/typeMap";

export function getFileIcon(fileName: string) {
	fileName = fileName.toLowerCase();

	// Check if fileName is directly an icon name
	if (fileIcons[fileName]) {
		return fileIcons[fileName];
	}

	const splitName = fileName.split(".");
	let iconName = "";

	while (splitName.length) {
		const curName = splitName.join(".");
		if (symbolFileNamesToIcons[curName]) {
			iconName = symbolFileNamesToIcons[curName] ?? "";
			break;
		}
		if (symbolFileExtensionsToIcons[curName]) {
			iconName = symbolFileExtensionsToIcons[curName] ?? "";
			break;
		}

		splitName.shift();
	}

	if (iconName === "") {
		iconName = "document";
	}
	let icon = fileIcons[iconName];
	if (!icon) {
		icon = fileIcons["document"];
	}
	return icon;
}
