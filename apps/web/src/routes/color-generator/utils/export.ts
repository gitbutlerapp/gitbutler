/**
 * Export utilities for color scales
 */

export function exportJSON(data: Record<string, Record<number, string>>) {
	const json = JSON.stringify(data, null, 2);
	downloadFile(json, 'color-scales.json', 'application/json');
}

export function exportCSS(data: Record<string, Record<number, string>>) {
	const css = generateCSSString(data);
	downloadFile(css, 'color-scales.css', 'text/css');
}

export function copyCSS(data: Record<string, Record<number, string>>): Promise<void> {
	let css = '';
	for (const [scaleId, scale] of Object.entries(data)) {
		for (const [shade, color] of Object.entries(scale)) {
			css += `--clr-core-${scaleId}-${shade}: ${color};\n`;
		}
	}
	return navigator.clipboard.writeText(css.trim());
}

function generateCSSString(data: Record<string, Record<number, string>>): string {
	let css = ':root {\n';
	for (const [scaleId, scale] of Object.entries(data)) {
		for (const [shade, color] of Object.entries(scale)) {
			css += `  --clr-core-${scaleId}-${shade}: ${color};\n`;
		}
	}
	css += '}';
	return css;
}

export function copyToClipboard(text: string): Promise<void> {
	return navigator.clipboard.writeText(text);
}

function downloadFile(content: string, filename: string, mimeType: string) {
	const blob = new Blob([content], { type: mimeType });
	const url = URL.createObjectURL(blob);
	const a = document.createElement('a');
	a.href = url;
	a.download = filename;
	a.click();
	URL.revokeObjectURL(url);
}
