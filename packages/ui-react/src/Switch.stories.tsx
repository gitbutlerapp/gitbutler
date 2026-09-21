import preview from "#storybook/preview";
import { Switch, type SwitchSize } from "./Switch.tsx";
import { useState } from "react";

const meta = preview.meta({
	component: Switch,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=699-614",
		},
	},
});

const Toggleable = ({ disabled = false, size }: { disabled?: boolean; size?: SwitchSize }) => {
	const [checked, setChecked] = useState(true);
	return <Switch checked={checked} disabled={disabled} size={size} onCheckedChange={setChecked} />;
};

export const Default = meta.story({
	render: () => <Toggleable />,
});

/** The size a settings row's switch wears, beside the row's 15px title. */
export const Large = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 8, alignItems: "center" }}>
			<Toggleable size="large" />
			<Switch checked disabled size="large" />
			<Switch checked={false} disabled size="large" />
		</div>
	),
});

export const Disabled = meta.story({
	render: () => (
		<div style={{ display: "flex", gap: 8, alignItems: "center" }}>
			<Switch checked disabled />
			<Switch checked={false} disabled />
		</div>
	),
});

/** Pairing a switch with a label is `SwitchButton`'s job — see its stories. */
