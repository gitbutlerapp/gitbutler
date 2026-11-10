// The corners a person can pick in place of Figma's, soft. Each setting moves the whole radius
// scale by the same 4px, so a control 4px inside a popup keeps its corners concentric. A
// step no bigger than the move, a checkbox's or a kbd's, moves half its size, so sharp never
// squares it. The role tokens alias the steps, so they follow on their own, as the override
// sits on the root element they're declared on. radius/full, the pills, never changes.

/** The corner settings a person can pick, besides Figma's, and how far each moves the scale. */
export const RADII = { sharp: -4, round: 4 };

const REM = 16;

/** How far `px`, a step of the scale, moves for a setting that moves the scale by `shift`. */
function moved(px, shift) {
	return px + Math.sign(shift) * (px > Math.abs(shift) ? Math.abs(shift) : px / 2);
}

/** `css` with an `:root[data-but-radius=…]` block for each setting after its token blocks. */
export function appendRadii(css) {
	const steps = [
		...css.matchAll(/^\s*(--but-radius-(?:xs|sm|md|lg|\d*xl)):\s*([\d.]+)rem;\s*$/gm),
	].map(([, name, rem]) => ({ name, px: Number(rem) * REM }));
	if (steps.length === 0) throw new Error("No radius steps declared in tokens.css.");
	const blocks = Object.entries(RADII).map(([setting, shift]) => {
		const lines = steps.map(({ name, px }) => `  ${name}: ${moved(px, shift) / REM}rem;`);
		return `:root[data-but-radius="${setting}"] {\n${lines.join("\n")}\n}\n`;
	});
	return `${css}\n/* The corners, set with data-but-radius on the root element; Figma's without it. */\n${blocks.join("\n")}`;
}
