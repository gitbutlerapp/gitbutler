import preview from "#storybook/preview";
import { Checkbox } from "./Checkbox.tsx";
import { useState } from "react";

const meta = preview.meta({
	component: Checkbox,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=699-584",
		},
	},
});

const Toggleable = ({ disabled = false }: { disabled?: boolean }) => {
	const [checked, setChecked] = useState(true);
	return <Checkbox checked={checked} disabled={disabled} onCheckedChange={setChecked} />;
};

export const Default = meta.story({
	render: () => <Toggleable />,
});

/**
 * Indeterminate stands for a folder with some of its files checked, so it is filled like a checked
 * box rather than shown as a third, emptier state.
 */
export const Indeterminate = meta.story({
	render: () => <Checkbox indeterminate />,
});

export const Disabled = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 8, alignItems: "center" }}>
			<Checkbox checked disabled />
			<Checkbox checked={false} disabled />
			<Checkbox indeterminate disabled />
		</div>
	),
});

/**
 * The box takes its ground from `--checkbox-bg`, left undeclared so a row can set it without a
 * specificity fight. Here the row is tinted to show the checkbox following it.
 */
export const OnATintedRow = meta.story({
	render: () => (
		<div
			style={{
				["--checkbox-bg" as string]: "var(--bg-2)",
				display: "flex",
				gap: 8,
				alignItems: "center",
				padding: 8,
				borderRadius: 6,
				backgroundColor: "var(--bg-2)",
			}}
		>
			<Checkbox checked={false} />
			<Checkbox checked />
			<Checkbox indeterminate />
		</div>
	),
});
