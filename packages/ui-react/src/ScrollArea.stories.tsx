import preview from "#storybook/preview";
import { ScrollArea } from "./ScrollArea.tsx";

const meta = preview.meta({
	component: ScrollArea,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2350-3012",
		},
	},
	args: { separator: false },
});

const rows = Array.from({ length: 40 }, (_, index) => `Row ${index + 1}`);

/** A pane shorter than its content: the thumb shows while it scrolls or is pointed at. */
export const Default = meta.story({
	args: { separator: false },
	render: (args) => (
		<ScrollArea {...args} style={{ height: 240, width: 280, border: "1px solid var(--border-2)" }}>
			{rows.map((row) => (
				<div key={row} className="text-13" style={{ padding: "6px 12px" }}>
					{row}
				</div>
			))}
		</ScrollArea>
	),
});

/**
 * With `separator`, a hairline crosses the top once the content is scrolled down, so the rows read
 * as passing under the header above.
 */
export const WithSeparator = meta.story({
	args: { separator: true },
	render: (args) => (
		<div
			style={{
				display: "flex",
				flexDirection: "column",
				height: 280,
				width: 280,
				border: "1px solid var(--border-2)",
			}}
		>
			<div className="text-13 text-bold" style={{ padding: "10px 12px" }}>
				Changes
			</div>
			<ScrollArea {...args} style={{ flexGrow: 1 }}>
				{rows.map((row) => (
					<div key={row} className="text-13" style={{ padding: "6px 12px" }}>
						{row}
					</div>
				))}
			</ScrollArea>
		</div>
	),
});
