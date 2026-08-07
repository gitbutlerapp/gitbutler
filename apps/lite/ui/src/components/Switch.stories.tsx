import preview from "#storybook/preview";
import { Switch } from "./Switch.tsx";
import { useState } from "react";

const meta = preview.meta({
	component: Switch,
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
			{/* The labeled-pill treatment: the container fades as a whole and
			    neutralizes the switch's own fade via its opacity variable. */}
			<span
				style={{
					display: "inline-flex",
					alignItems: "center",
					gap: 6,
					height: 28,
					paddingInline: 8,
					border: "1px solid var(--border-2)",
					borderRadius: "var(--radius-button)",
					opacity: 0.5,
					["--disabled-opacity" as string]: "100%",
				}}
				className="text-13"
			>
				<Switch checked disabled />
				Auto-merge
			</span>
		</div>
	),
});

const LabeledSwitch = () => {
	const [checked, setChecked] = useState(true);
	return (
		<span
			style={{
				display: "inline-flex",
				alignItems: "center",
				gap: 6,
				height: 28,
				paddingInline: 8,
				border: "1px solid var(--border-2)",
				borderRadius: "var(--radius-button)",
			}}
			className="text-13"
		>
			<Switch id="story-auto-merge" checked={checked} onCheckedChange={setChecked} />
			{/* for-association, not a wrapping label: a wrapper forwards clicks
			    back to the switch button and every press double-toggles. */}
			<label htmlFor="story-auto-merge" style={{ cursor: "pointer" }}>
				Auto-merge
			</label>
		</span>
	);
};

/** The PR header pairing: switch plus label in outline-button chrome. */
export const WithLabel = meta.story({
	render: () => <LabeledSwitch />,
});
