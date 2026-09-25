// The design rules a machine can hold the components to. Each message says
// what to use instead, because the reader is as often an agent as a person,
// and an error that only forbids gets worked around. The rules and their
// exceptions are DESIGN.md's; when one changes there, change it here.

// Not a fragment id: url(#clip) is a reference, not a colour.
const hex = "/(?<!url\\()#[0-9a-f]{3,8}\\b/i";

export default {
	rules: {
		// Colours are tokens. A raw value can't follow the theme and isn't in Figma.
		"declaration-property-value-disallowed-list": [
			{
				// Everything but masks: a mask's colour paints nothing, only its alpha counts.
				"/^(?!(-webkit-)?mask)/": [hex],
			},
			{
				/** @param {string} property */
				message: (property) =>
					`Raw colour in ${property}. Use a @gitbutler/design-core token, e.g. var(--text-2) or var(--fill-gray-bg); ` +
					"the only raw colours are the file icons' brand colours, in FileIcon.module.css (DESIGN.md, Icons).",
			},
		],
		"function-disallowed-list": [
			["rgb", "rgba", "hsl", "hsla"],
			{
				/** @param {string} name */
				message: (name) =>
					`${name}() builds a colour from numbers. Use a design-core token, or color-mix()/light-dark() over tokens (DESIGN.md).`,
			},
		],
		"declaration-property-value-allowed-list": [
			{
				// The type scale: 11 to 16, as Figma's Base/ and Body/ styles. Lite's Markdown
				// headings go to 18 in its own CSS, outside this package.
				"font-size": ["/^1[1-6]px$/", "inherit", "/^var\\(/"],
				// Radii come from the scale, or from a token with padding subtracted (DESIGN.md, Radius).
				"border-radius": ["/^var\\(--radius-/", "/^calc\\(/", "0", "inherit"],
			},
			{
				/**
				 * @param {string} property
				 * @param {string} value
				 */
				message: (property, value) =>
					property === "font-size"
						? `font-size ${value} is off the scale. Sizes are 11px to 16px, matching Figma's Base/ and Body/ styles; ` +
							"prefer the text-NN class on the element (DESIGN.md)."
						: `border-radius ${value} is not a token. Use var(--radius-xs|sm|md|lg|xl|2xl|full), a semantic one ` +
							"(--radius-control, --radius-card, --radius-popup, --radius-section), calc() from one, or 0; a circle or a pill is --radius-full (DESIGN.md, Radius).",
			},
		],
	},
	overrides: [
		{
			// Language and filetype glyphs carry their own brand colours by design (DESIGN.md, Icons).
			files: ["src/FileIcon.module.css"],
			rules: { "declaration-property-value-disallowed-list": null },
		},
	],
};
