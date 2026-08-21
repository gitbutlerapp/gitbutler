import preview from "#storybook/preview";
import { SwitchButton, type SwitchButtonVariant } from "./SwitchButton.tsx";
import { useState } from "react";

const meta = preview.meta({
	component: SwitchButton,
});

const variants = ["ghost", "outline"] as const satisfies ReadonlyArray<SwitchButtonVariant>;

const Toggleable = ({
	label,
	variant,
	initial = false,
}: {
	label: string;
	variant?: SwitchButtonVariant;
	initial?: boolean;
}) => {
	const [checked, setChecked] = useState(initial);
	return (
		<SwitchButton label={label} variant={variant} checked={checked} onCheckedChange={setChecked} />
	);
};

export const Playground = meta.story({
	argTypes: {
		variant: { control: "radio", options: variants },
		label: { control: "text" },
		disabled: { control: "boolean" },
	},
	args: {
		label: "Draft",
		variant: "ghost",
		disabled: false,
	},
	render: ({ label, variant, disabled }) => (
		<SwitchButton label={label} variant={variant} disabled={disabled} defaultChecked />
	),
});

/** Ghost sits inside a surface that already has chrome; outline stands alone. */
export const Variants = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12, alignItems: "center" }}>
			<Toggleable label="Draft" variant="ghost" />
			<Toggleable label="Auto-merge" variant="outline" initial />
		</div>
	),
});

/** The pill fades as a whole rather than compounding the switch's own fade. */
export const Disabled = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 12, alignItems: "center" }}>
			<SwitchButton label="Draft" disabled />
			<SwitchButton label="Draft" disabled defaultChecked />
			<SwitchButton label="Auto-merge" variant="outline" disabled defaultChecked />
		</div>
	),
});

/**
 * The label shares the switch's hit area, so a click anywhere in the pill —
 * including on the switch itself — toggles exactly once.
 */
export const LongLabel = meta.story({
	render: () => <Toggleable label="Delete the branch after merging" variant="outline" />,
});
