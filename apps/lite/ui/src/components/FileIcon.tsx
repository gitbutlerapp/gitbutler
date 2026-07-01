import styles from "./FileIcon.module.css";
import { classes } from "#ui/components/classes.ts";
import { symbolFileExtensionsToIcons, symbolFileNamesToIcons } from "./file-icons/typeMap.ts";
import { ComponentProps, FC } from "react";

const modules = import.meta.glob<string>("./file-icons/svg/*.svg", {
	query: "?raw",
	import: "default",
	eager: true,
});

const fileIcons: Record<string, string> = {};

for (const [modulePath, svg] of Object.entries(modules)) {
	const name = modulePath.replace(/^.*\//, "").replace(/\.svg$/, "");
	fileIcons[name] = svg;
}

const getFileIcon = (fileName: string): string => {
	const parts = fileName.toLowerCase().split(".");

	for (let index = 0; index < parts.length; index++) {
		const suffix = parts.slice(index).join(".");
		const iconName = symbolFileNamesToIcons[suffix] ?? symbolFileExtensionsToIcons[suffix];
		const icon = iconName !== undefined ? fileIcons[iconName] : undefined;
		if (icon !== undefined) return icon;
	}

	return fileIcons.document ?? "";
};

type Props = {
	fileName: string;
} & ComponentProps<"i">;

export const FileIcon: FC<Props> = ({ fileName, ...props }) => (
	<i
		{...props}
		className={classes(props.className, styles.fileIcon)}
		aria-hidden
		// oxlint-disable-next-line react/no-danger -- SVGs are bundled app assets.
		dangerouslySetInnerHTML={{ __html: getFileIcon(fileName) }}
	/>
);
