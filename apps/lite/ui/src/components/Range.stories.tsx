import preview from "#storybook/preview";
import { Range } from "./Range.tsx";

const meta = preview.meta({
	component: Range,
	parameters: {
		design: {
			type: "figma",
			url: "https://www.figma.com/design/cqdnAotT8n9op8WGYLOHg4/%E2%9A%9B%EF%B8%8F-Lite-Core?node-id=2071-3993",
		},
	},
	args: {
		label: "Label",
		defaultValue: 50,
		style: { width: 170 },
	},
});

/** The label at one end of the header and the value at the other; the fill reads the value as a
 * proportion before the number does. */
export const Default = meta.story({});

/** A pair of values grows a second thumb, and the fill runs between the two. */
export const Span = meta.story({
	args: { label: "Range", defaultValue: [15, 55] },
});

/** A scale under the track, each mark centred on where the thumb sits at that value. */
export const WithMarks = meta.story({
	args: { defaultValue: 80, marks: [0, 20, 40, 60, 80, 100] },
});

/** Marks with labels, for stops whose numbers alone wouldn't say what they mean: here each step
 * of the range stands for a duration, and the first for none at all. */
export const LabelledMarks = meta.story({
	args: {
		label: undefined,
		"aria-label": "Auto-fetch frequency",
		defaultValue: 2,
		min: 0,
		max: 5,
		step: 1,
		marks: [
			{ value: 0, label: "Off" },
			{ value: 1, label: "5min" },
			{ value: 2, label: "15min" },
			{ value: 3, label: "30min" },
			{ value: 4, label: "1h" },
			{ value: 5, label: "2h" },
		],
		style: { width: 400 },
	},
});

/** Steps snap the thumb, and the format gives the value its unit. */
export const Stepped = meta.story({
	args: {
		label: "Opacity",
		defaultValue: 0.6,
		min: 0,
		max: 1,
		step: 0.1,
		format: { style: "percent" },
	},
});

export const Disabled = meta.story({
	args: { defaultValue: 50, disabled: true },
});

/** Without a header, for a range the surface already names — a row in a settings table. */
export const Unlabelled = meta.story({
	args: { label: undefined, "aria-label": "Label", defaultValue: 50 },
});
