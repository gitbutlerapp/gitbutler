import preview from "#storybook/preview";
import { FolderIcon } from "./FolderIcon.tsx";

const meta = preview.meta({
	component: FolderIcon,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Core?node-id=229-2700",
		},
	},
});

/** The mark at the size it was drawn, 17×14. */
export const Default = meta.story({});

/** Callers size it with CSS, and it keeps its proportions in whatever box it gets. */
export const Sizes = meta.story({
	render: () => (
		<div style={{ display: "flex", alignItems: "end", gap: 16 }}>
			{[12, 14, 17, 24, 34].map((width) => (
				<FolderIcon key={width} style={{ width, height: "auto" }} />
			))}
		</div>
	),
});
