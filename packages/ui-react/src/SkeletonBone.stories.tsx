import preview from "#storybook/preview";
import { SkeletonBone } from "./SkeletonBone.tsx";

const FIGMA = "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core";

const meta = preview.meta({
	title: "Content & status/SkeletonBone",
	id: "components-skeleton-bone",
	component: SkeletonBone,
	parameters: {
		design: { type: "figma", url: `${FIGMA}?node-id=3621-1525` },
	},
	decorators: [
		(Story) => (
			<div style={{ width: 320, padding: 24 }}>
				<Story />
			</div>
		),
	],
});

/** A line of `text-13` that is still loading. */
export const Default = meta.story({
	parameters: { design: { type: "figma", url: `${FIGMA}?node-id=3614-81908` } },
	args: { width: 106, className: "text-13" },
});

/** An avatar or an icon that is still loading, at the size it will be drawn. */
export const Circle = meta.story({
	parameters: { design: { type: "figma", url: `${FIGMA}?node-id=3621-1523` } },
	args: { shape: "circle", size: 16 },
});

/** A bone takes the line height of the text style it is given, so the text replaces it in place. */
export const MatchesItsText = meta.story({
	parameters: { design: { type: "figma", url: `${FIGMA}?node-id=3640-1514` } },
	render: () => (
		<div
			style={{
				display: "grid",
				gridTemplateColumns: "auto 1fr",
				alignItems: "start",
				columnGap: 16,
				rowGap: 8,
			}}
		>
			{["text-11", "text-12", "text-13", "text-14", "text-15"].map((style) => [
				<span key={`${style}-text`} className={style}>
					{style}
				</span>,
				<SkeletonBone key={`${style}-bone`} className={style} width={80} />,
			])}
		</div>
	),
});

/** Commits loading in a branch: each row drawn as it will look, with a bone per piece. */
export const LoadingCommits = meta.story({
	parameters: { design: { type: "figma", url: `${FIGMA}?node-id=3640-1535` } },
	render: () => (
		<div aria-busy style={{ display: "flex", flexDirection: "column", gap: 12 }}>
			{["72%", "54%", "64%"].map((width) => (
				<div key={width} style={{ display: "flex", alignItems: "flex-start", gap: 8 }}>
					<SkeletonBone shape="circle" size={16} />
					<div style={{ display: "flex", flex: 1, flexDirection: "column", minWidth: 0 }}>
						<SkeletonBone className="text-13" width={width} />
						<SkeletonBone className="text-12" width={96} />
					</div>
				</div>
			))}
		</div>
	),
});
