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
