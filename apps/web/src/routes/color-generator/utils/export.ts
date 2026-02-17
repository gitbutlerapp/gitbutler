/**
 * Export utilities for color scales
 */

export async function copyCSS(
	data: Record<string, Record<number, string>>,
	artColorsLight: Record<string, { h: number; s: number; l: number }>,
	artColorsDark: Record<string, { h: number; s: number; l: number }>,
): Promise<void> {
	let lightCss = "";
	let darkCss = "";

	// Add core color scales (only in light mode, no dark variants)
	for (const [scaleId, scale] of Object.entries(data)) {
		for (const [shade, color] of Object.entries(scale)) {
			// Skip 0 and 100 shades for non-gray colors
			const shadeNum = Number(shade);
			if (scaleId !== "gray" && (shadeNum === 0 || shadeNum === 100)) {
				continue;
			}
			lightCss += `  --clr-core-${scaleId}-${shade}: ${color};\n`;
		}
	}

	// Add art colors (with separate light and dark values)
	for (const [colorId, colorLight] of Object.entries(artColorsLight)) {
		const lightValue = `hsl(${colorLight.h}, ${colorLight.s}%, ${colorLight.l}%)`;
		lightCss += `  --clr-${colorId}: ${lightValue};\n`;

		const colorDark = artColorsDark[colorId];
		const darkValue = `hsl(${colorDark.h}, ${colorDark.s}%, ${colorDark.l}%)`;
		darkCss += `  --clr-${colorId}: ${darkValue};\n`;
	}

	let css = `:root {\n${lightCss.trimEnd()}\n}`;
	if (darkCss.trim()) {
		css += `\n\n:root.dark {\n${darkCss.trimEnd()}\n}`;
	}
	return await navigator.clipboard.writeText(css);
}

export async function copyJSON(
	data: Record<string, Record<number, string>>,
	artColorsLight: Record<string, { h: number; s: number; l: number }>,
	artColorsDark: Record<string, { h: number; s: number; l: number }>,
): Promise<void> {
	const dtcgTokens: Record<string, any> = {
		"clr-core": {},
	};

	for (const [scaleId, scale] of Object.entries(data)) {
		dtcgTokens["clr-core"][scaleId] = {};
		for (const [shade, color] of Object.entries(scale)) {
			// Skip 0 and 100 shades for non-gray colors
			const shadeNum = Number(shade);
			if (scaleId !== "gray" && (shadeNum === 0 || shadeNum === 100)) {
				continue;
			}
			dtcgTokens["clr-core"][scaleId][shade] = {
				$value: color,
				$type: "color",
				$extensions: {
					mode: {},
				},
			};
		}
	}

	// Add art colors
	dtcgTokens["clr"] = {};
	for (const [colorId, colorLight] of Object.entries(artColorsLight)) {
		const lightValue = `hsl(${colorLight.h}, ${colorLight.s}%, ${colorLight.l}%)`;
		const colorDark = artColorsDark[colorId];
		const darkValue = `hsl(${colorDark.h}, ${colorDark.s}%, ${colorDark.l}%)`;

		dtcgTokens["clr"][colorId] = {
			$value: lightValue,
			$type: "color",
			$extensions: {
				mode: {
					light: lightValue,
					dark: darkValue,
				},
			},
		};
	}

	const json = JSON.stringify(dtcgTokens, null, 2);
	return await navigator.clipboard.writeText(json);
}

export async function copyToClipboard(text: string): Promise<void> {
	return await navigator.clipboard.writeText(text);
}
