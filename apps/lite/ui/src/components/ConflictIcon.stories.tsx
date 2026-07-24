import preview from "#storybook/preview";
import { ConflictIcon } from "./ConflictIcon.tsx";

const meta = preview.meta({
	component: ConflictIcon,
	argTypes: {
		variant: {
			control: "select",
			options: ["conflict", "plus", "minus"],
		},
		size: {
			control: { type: "range", min: 8, max: 128, step: 4 },
		},
	},
	args: {
		variant: "conflict",
		size: 16,
	},
});

export const Default = meta.story({});

export const AllVariants = meta.story({
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
