const GENERIC_FAMILY = { base: "sans-serif", mono: "monospace", accent: "serif" };

export function transformToken(token, mode) {
	const modeValue = token.mode?.[mode];
	if (!modeValue) return;

	if (token.$type === "color") {
		if (modeValue.aliasOf) return;
		const val = modeValue.$value;
		if (val && val.components && val.colorSpace === "srgb") {
			const [r, g, b] = val.components;
			const alpha = val.alpha ?? 1;
			const max = Math.max(r, g, b);
			const min = Math.min(r, g, b);
			const d = max - min;
			let h = 0;
			const l = (max + min) / 2;
			const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
			if (d !== 0) {
				switch (max) {
					case r:
						h = (((g - b) / d) % 6) * 60;
						break;
					case g:
						h = ((b - r) / d + 2) * 60;
						break;
					default:
						h = ((r - g) / d + 4) * 60;
						break;
				}
				if (h < 0) h += 360;
			}
			const hStr = Math.round(h);
			const sStr = (s * 100).toFixed(1);
			const lStr = (l * 100).toFixed(1);
			if (alpha < 1) {
				return `hsl(${hStr} ${sStr}% ${lStr}% / ${parseFloat(alpha.toFixed(2))})`;
			}
			return `hsl(${hStr} ${sStr}% ${lStr}%)`;
		}
	}

	// Figma holds one family per token. The generic one after it is what shows
	// while the font loads (fonts.css swaps) and where it is missing.
	if (token.$type === "fontFamily") {
		const generic = GENERIC_FAMILY[token.id.split(".").pop()];
		if (generic) {
			const families = [modeValue.$value].flat().map((family) => `"${family}"`);
			return [...families, generic].join(", ");
		}
	}

	if (token.$type === "dimension") {
		// Leave aliases to Terrazzo, which writes them as var(--target), so
		// --gap-buttons reads as a role on the scale rather than a bare 0.5rem.
		if (modeValue.aliasOf) return;
		const val = modeValue.$value;
		if (val && val.unit === "px" && val.value !== 0) {
			return `${val.value / 16}rem`;
		}
	}
}

// A token's CSS name: its path with dashes, under the but- prefix. The raw
// palettes keep their own names (--clr-*, --scale-*): they are steps, not roles,
// and components don't read them. A group's own token ($root, as primary is to
// primary/foreground) takes the group's name.
export function cssVarName(path) {
	const name = path.replaceAll(".$root", "").replaceAll(".", "-");
	return /^(clr|scale)-/.test(name) ? `--${name}` : `--but-${name}`;
}

export function stripCollectionPrefix(id) {
	const collections = ["core", "semantic"];
	for (const collection of collections) {
		if (id.startsWith(collection + ".")) {
			return cssVarName(id.slice(collection.length + 1));
		}
	}
}
