import {
	symbolFileExtensionsToIcons,
	symbolFileNamesToIcons,
} from "$lib/components/file/icon/typeMap";

const modules = import.meta.glob<string>("./svg/*.svg", {
	query: "?raw",
	import: "default",
	eager: true,
});

export const fileIcons: Record<string, string> = {};

for (const [modulePath, svg] of Object.entries(modules)) {
	const name = modulePath.replace(/^.*\//, "").replace(/\.svg$/, "");
	fileIcons[name] = svg;
}

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
