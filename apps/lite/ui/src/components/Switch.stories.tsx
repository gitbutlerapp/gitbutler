import preview from "#storybook/preview";
import { Switch } from "./Switch.tsx";
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

const Toggleable = ({ disabled = false }: { disabled?: boolean }) => {
	const [checked, setChecked] = useState(true);
	return <Switch checked={checked} disabled={disabled} onCheckedChange={setChecked} />;
};

export const Default = meta.story({
	render: () => <Toggleable />,
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
