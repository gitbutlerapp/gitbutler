import preview from "#storybook/preview";
import { type GraphSegmentGlyph, GraphSegment } from "./GraphSegment.tsx";

const glyphs = [
	"parent",
	"horizontal",
	"space",
	"forkLeft",
	"forkRight",
	"notch",
	"forkBoth",
	"mergeLeft",
	"mergeRight",
	"mergeBoth",
	"joinLeft",
	"joinRight",
	"joinBoth",
	"hook",
	"commit",
	"group",
] satisfies Array<GraphSegmentGlyph>;

const meta = preview.meta({
	component: GraphSegment,
	argTypes: {
		glyph: {
			control: { type: "select" },
			options: glyphs,
		},
	},
});

export const Default = meta.story({
	args: {
		glyph: "parent",
		status: "LocalOnly",
	},
	render: (args) => (
		<div
			style={{
				height: 100,
				backgroundColor: "var(--bg-2)",
				display: "flex",
			}}
		>
			<GraphSegment {...args} />
		</div>
	),
});

export const AllGlyphs = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 16 }}>
			{glyphs.map((glyph) => (
				<div
					key={glyph}
					style={{
						display: "flex",
						alignItems: "flex-start",
						flexDirection: "column",
						gap: 12,
					}}
				>
					<div style={{ backgroundColor: "var(--bg-2)", display: "flex" }}>
						<GraphSegment glyph={glyph} status="LocalOnly" />
					</div>
					<div style={{ fontSize: 10 }}>{glyph}</div>
				</div>
			))}
		</div>
	),
});
