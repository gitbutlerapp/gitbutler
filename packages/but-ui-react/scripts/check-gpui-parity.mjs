#!/usr/bin/env node

/**
 * Checks that the GPUI client's components (crates/but-ui-gpui) keep the variants and sizes of
 * their but-ui-react counterparts, as that crate's CLAUDE.md asks.
 *
 * PARITY says, for each value of a component's variant or size type here, how the crate draws
 * it, or `null` where it doesn't yet. The check fails when:
 *
 * - a value is added here, or renamed, and PARITY doesn't say how GPUI draws it;
 * - PARITY names a value that's gone from here;
 * - PARITY names something the crate's file doesn't have, as after a rename there;
 * - an enum PARITY names has a variant no value here maps to: GPUI drawing what React can't.
 *
 * So a new variant on either side fails until the other gets it, or until it's written down
 * here as a gap. The gaps are listed on every run.
 *
 * How GPUI draws a value, one or more of:
 * - `Enum::Variant`, a variant of a `pub enum` in the file;
 * - `fn name`, a `pub fn` in the file, a builder or the component's own function;
 * - `default`, what the component is with nothing asked of it;
 * - any other text, which the file has to contain, as an argument (`small: bool`).
 *
 * Usage: node scripts/check-gpui-parity.mjs   (exit 1 on any problem)
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const src = join(dirname(fileURLToPath(import.meta.url)), "..", "src");
const crate = join(src, "..", "..", "..", "crates", "but-ui-gpui", "src");

/**
 * @type {{
 *   component: string;
 *   react: string;
 *   type: string;
 *   keys?: true;
 *   gpui: string;
 *   values: Record<string, string[] | null>;
 * }[]}
 */
const PARITY = [
	{
		component: "Button",
		react: "Button.tsx",
		type: "ButtonVariant",
		gpui: "button.rs",
		values: {
			pop: ["ButtonStyle::Pop"],
			gray: ["ButtonStyle::Gray"],
			outline: ["ButtonStyle::Outline"],
			"outline-inverted": ["ButtonStyle::Outline", "fn inverted"],
			danger: ["ButtonStyle::Danger"],
			"danger-solid": null,
			// Its `tint` takes a banner's colour, a different thing from these outlines.
			info: null,
			warning: null,
			success: null,
			ghost: ["ButtonStyle::Ghost"],
			"ghost-inverted": ["ButtonStyle::Ghost", "fn inverted"],
		},
	},
	{
		component: "Button",
		react: "Button.tsx",
		type: "ButtonSize",
		gpui: "button.rs",
		values: { regular: ["default"], medium: null, small: ["fn small"] },
	},
	{
		component: "DropdownButton",
		react: "DropdownButton.tsx",
		type: "DropdownButtonVariant",
		gpui: "button.rs",
		values: {
			pop: ["fn dropdown_button", "ButtonStyle::Pop"],
			gray: ["fn dropdown_button", "ButtonStyle::Gray"],
			outline: ["fn dropdown_button", "ButtonStyle::Outline"],
		},
	},
	{
		component: "Badge",
		react: "Badge.tsx",
		type: "BadgeVariant",
		gpui: "badge.rs",
		values: {
			fillGray: null,
			lightGray: ["fn count_badge"],
			safe: null,
			warn: null,
			danger: null,
			purple: null,
			blue: null,
			pop: null,
		},
	},
	{
		component: "Badge",
		react: "Badge.tsx",
		type: "BadgeSize",
		gpui: "badge.rs",
		values: { regular: ["fn count_badge"], large: null },
	},
	{
		component: "SegmentedControl",
		react: "SegmentedControl.tsx",
		type: "SegmentedControlSize",
		gpui: "segmented.rs",
		values: { regular: ["fn segmented"], small: ["fn segmented", "small: bool"] },
	},
	{
		component: "Snackbar",
		react: "Snackbar.tsx",
		type: "SnackbarVariant",
		gpui: "snackbar.rs",
		values: {
			info: ["fn snackbar"],
			warning: ["fn snackbar", "warning: bool"],
			danger: null,
			safe: null,
		},
	},
	{
		component: "Switch",
		react: "Switch.tsx",
		type: "SwitchSize",
		gpui: "switch.rs",
		// GPUI's one switch is 30×18, the size of neither.
		values: { default: null, large: null },
	},
	{
		component: "Modal",
		react: "Popup.tsx",
		type: "ModalSize",
		gpui: "modal.rs",
		values: {
			xsmall: null,
			small: null,
			medium: ["ModalSize::Medium"],
			large: ["ModalSize::Large"],
		},
	},
	{
		component: "Illustration",
		react: "illustrations.ts",
		type: "illustrations",
		keys: true,
		gpui: "illustration.rs",
		values: {
			cactus: ["Illustration::Cactus"],
			"id-card": null,
			papers: ["Illustration::Papers"],
			sleep: null,
			terminal: ["Illustration::Terminal"],
			waving: ["Illustration::Waving"],
		},
	},
];

/** @param {string} path */
const read = (path) => readFileSync(path, "utf8");

/** A file's code without its comments, which name values they don't define. */
const uncommented = (/** @type {string} */ code) =>
	code.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");

/** The string values of `export type <name> = "a" | "b";`, or the keys of `const <name> = {`. */
const reactValues = (/** @type {(typeof PARITY)[number]} */ entry) => {
	const code = uncommented(read(join(src, entry.react)));
	if (entry.keys !== true) {
		const union = new RegExp(`export type ${entry.type} =([^;]*);`).exec(code)?.[1];
		if (union === undefined) return undefined;
		return [...union.matchAll(/"([^"]+)"/g)].map(([, value]) => value);
	}
	const object = new RegExp(`export const ${entry.type} = \\{([^}]*)\\}`).exec(code)?.[1];
	if (object === undefined) return undefined;
	return [...object.matchAll(/^\s*"?([\w-]+)"?\s*[:,]/gm)].map(([, key]) => key);
};

/** The variants of `pub enum <name> { … }` in a Rust file. */
const rustVariants = (/** @type {string} */ code, /** @type {string} */ name) => {
	const body = new RegExp(`pub enum ${name} \\{([\\s\\S]*?)\\n\\}`).exec(code)?.[1];
	if (body === undefined) return undefined;
	return [...uncommented(body).matchAll(/^\s*([A-Z]\w*)\s*[,({]/gm)].map(([, variant]) => variant);
};

/** @type {string[]} */
const problems = [];
/** @type {string[]} */
const gaps = [];
/** Each enum named, by file, and the variants some value maps to. @type {Map<string, Set<string>>} */
const claimed = new Map();

for (const entry of PARITY) {
	const what = entry.type;
	const values = reactValues(entry);
	if (values === undefined) {
		problems.push(`${entry.react} has no ${what}.`);
		continue;
	}
	for (const value of values.filter((value) => !(value in entry.values))) {
		problems.push(
			`${what} "${value}" (${entry.react}): say how ${entry.gpui} draws it, or null if it doesn't yet.`,
		);
	}
	const rust = read(join(crate, entry.gpui));
	for (const [value, drawn] of Object.entries(entry.values)) {
		if (!values.includes(value))
			problems.push(`${what} has no "${value}" any more (${entry.react}).`);
		if (drawn === null) {
			gaps.push(`${entry.component} ${value}`);
			continue;
		}
		for (const part of drawn) {
			const variantOf = /^(\w+)::(\w+)$/.exec(part);
			const fn = /^fn (\w+)$/.exec(part)?.[1];
			const found =
				part === "default" ||
				(variantOf !== null
					? rustVariants(rust, variantOf[1])?.includes(variantOf[2])
					: fn !== undefined
						? new RegExp(`pub fn ${fn}\\b`).test(rust)
						: rust.includes(part));
			if (!found) problems.push(`${entry.gpui} has no ${part}, which draws ${what} "${value}".`);
			if (variantOf !== null) {
				const key = `${entry.gpui} ${variantOf[1]}`;
				claimed.set(key, (claimed.get(key) ?? new Set()).add(variantOf[2]));
			}
		}
	}
}

for (const [key, variants] of claimed) {
	const [file, enumName] = key.split(" ");
	const unmapped = (rustVariants(read(join(crate, file)), enumName) ?? []).filter(
		(variant) => !variants.has(variant),
	);
	for (const variant of unmapped) {
		problems.push(
			`${file}'s ${enumName}::${variant} draws no but-ui-react value: add it there, or map it here.`,
		);
	}
}

if (problems.length > 0) {
	process.stderr.write("check-gpui-parity:\n");
	for (const problem of problems) process.stderr.write(`  ${problem}\n`);
	process.exit(1);
}
process.stdout.write(`check-gpui-parity: GPUI doesn't draw yet: ${gaps.join(", ")}.\n`);
