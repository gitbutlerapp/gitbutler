import { stories } from "../docs/stories.ts";
import { ConflictIcon } from "./ConflictIcon.tsx";

const meta = stories({
	title: "Content & status/ConflictIcon",
	id: "components-conflicticon",
	component: ConflictIcon,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%F0%9F%8D%91-But-UI?node-id=2134-5924",
		},
	},
	args: {
		variant: "conflict",
		size: 16,
	},
});

export const Default = meta.story({
	args: {
		variant: "conflict",
		size: 16,
	},
});

export const AllVariants = meta.story({
	args: {
		variant: "conflict",
		size: 16,
	},
	render: (args) => (
		<div style={{ display: "flex", gap: 16, alignItems: "center" }}>
			{(["conflict", "plus", "minus"] as const).map((variant) => (
				<div
					key={variant}
					style={{ display: "flex", flexDirection: "column", gap: 8, alignItems: "center" }}
				>
					<ConflictIcon variant={variant} size={args.size} />
					<span style={{ fontSize: 11, opacity: 0.5 }}>{variant}</span>
				</div>
			))}
		</div>
	),
});
