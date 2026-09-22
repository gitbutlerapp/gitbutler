import preview from "#storybook/preview";
import { NumberField } from "./NumberField.tsx";

const meta = preview.meta({
	component: NumberField,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2067-3770",
		},
	},
	args: {
		label: "Label",
		placeholder: "0",
		style: { width: 100 },
	},
});

/** Empty: the input reads the placeholder, and the steppers count from zero. */
export const Default = meta.story({});

export const WithValue = meta.story({
	args: { defaultValue: 99 },
});

/** A range: the stepper that would leave it is disabled, and a typed value is clamped on blur. */
export const WithRange = meta.story({
	args: { label: "Tab size", defaultValue: 8, min: 1, max: 8, step: 1 },
});

export const Disabled = meta.story({
	args: { defaultValue: 99, disabled: true },
});
