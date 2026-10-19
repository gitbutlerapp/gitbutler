import preview from "#storybook/preview";
import { FileIcon } from "./FileIcon.tsx";
import { symbolFileExtensionsToIcons, symbolFileNamesToIcons } from "./file-icons/typeMap.ts";

/** One file name per icon, the first the map sends to it, so each drawn icon shows once. */
const samples = new Map<string, string>();
for (const [extension, icon] of Object.entries(symbolFileExtensionsToIcons))
	if (!samples.has(icon)) samples.set(icon, `file.${extension}`);
for (const [name, icon] of Object.entries(symbolFileNamesToIcons))
	if (!samples.has(icon)) samples.set(icon, name);

const meta = preview.meta({
	title: "Icons/FileIcon",
	id: "components-fileicon",
	component: FileIcon,
	args: {
		fileName: "FileIcon.tsx",
	},
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=229-2589",
		},
	},
});

/** Type a file name; the icon follows its name or extension, and a type it doesn't know gets `document`. */
export const Default = meta.story({});

/** Every icon a file name can reach, under the icon's name in ⚛️ Core. */
export const AllIcons = meta.story({
	render: () => (
		<div
			className="text-11"
			style={{
				display: "grid",
				gridTemplateColumns: "repeat(auto-fill, minmax(112px, 1fr))",
				gap: 12,
				padding: 8,
			}}
		>
			{[...samples]
				.sort(([a], [b]) => a.localeCompare(b))
				.map(([icon, fileName]) => (
					<div
						key={icon}
						title={fileName}
						style={{ display: "flex", alignItems: "center", gap: 6 }}
					>
						<FileIcon fileName={fileName} />
						<span>{icon}</span>
					</div>
				))}
		</div>
	),
});
