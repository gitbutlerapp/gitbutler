/**
 * Export utilities for color scales
 */

export async function copyCSS(data: Record<string, Record<number, string>>): Promise<void> {
	let css = '';
	for (const [scaleId, scale] of Object.entries(data)) {
		for (const [shade, color] of Object.entries(scale)) {
			// Skip 0 and 100 shades for non-gray colors
			const shadeNum = Number(shade);
			if (scaleId !== 'gray' && (shadeNum === 0 || shadeNum === 100)) {
				continue;
			}
			css += `--clr-core-${scaleId}-${shade}: ${color};\n`;
		}
	}
	return await navigator.clipboard.writeText(css.trim());
}

export async function copyJSON(data: Record<string, Record<number, string>>): Promise<void> {
	const dtcgTokens: Record<string, any> = {
		'clr-core': {}
	};

	for (const [scaleId, scale] of Object.entries(data)) {
		dtcgTokens['clr-core'][scaleId] = {};
		for (const [shade, color] of Object.entries(scale)) {
			// Skip 0 and 100 shades for non-gray colors
			const shadeNum = Number(shade);
			if (scaleId !== 'gray' && (shadeNum === 0 || shadeNum === 100)) {
				continue;
			}
			dtcgTokens['clr-core'][scaleId][shade] = {
				$value: color,
				$type: 'color'
			};
		}
	}

	const json = JSON.stringify(dtcgTokens, null, 2);
	return await navigator.clipboard.writeText(json);
}

export async function copyToClipboard(text: string): Promise<void> {
	return await navigator.clipboard.writeText(text);
}
