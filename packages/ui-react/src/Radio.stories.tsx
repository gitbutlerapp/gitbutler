import preview from "#storybook/preview";
import { Radio } from "./Radio.tsx";
import { RadioGroup } from "@base-ui/react";

const meta = preview.meta({
	component: Radio,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=699-601",
		},
	},
});

const options = ["Merge", "Rebase", "Squash"];

/** Radios in Base UI's `RadioGroup`, each with its label sharing the hit area. */
export const Default = meta.story({
	render: () => (
		<RadioGroup
			defaultValue="Merge"
			aria-label="Merge method"
			style={{ display: "flex", flexDirection: "column", gap: 8 }}
		>
			{options.map((option) => (
				<label
					key={option}
					className="text-13"
					style={{ display: "flex", gap: 8, alignItems: "center" }}
				>
					<Radio value={option} />
					{option}
				</label>
			))}
		</RadioGroup>
	),
});

export const Disabled = meta.story({
	render: () => (
		<RadioGroup
			defaultValue="on"
			disabled
			aria-label="Disabled"
			style={{ display: "flex", gap: 8, alignItems: "center" }}
		>
			<Radio value="on" aria-label="Checked" />
			<Radio value="off" aria-label="Unchecked" />
		</RadioGroup>
	),
});
