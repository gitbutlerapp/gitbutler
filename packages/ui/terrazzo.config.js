import { defineConfig } from '@terrazzo/cli';
import css from '@terrazzo/plugin-css';

function pxToRem(token) {
	if (token.$type === 'dimension') {
		if (token.$value.value === 'px') {
			return token.$value.slice(0, -2) / 16 + 'rem';
		}
	}
}

function clearFxPrefix(id) {
	if (id.includes('fx.')) {
		return id.replace('fx.', '').replace('.', '-');
	}
}

export default defineConfig({
	tokens: './src/lib/data/design-tokens.json',
	outDir: './src/styles/core',
	plugins: [
		css({
			filename: 'design-tokens.css',
			modeSelectors: [
				{
					mode: 'dark',
					selectors: [':root.dark']
				}
			],
			p3: false,
			transform: pxToRem,
			generateName(variableId) {
				return clearFxPrefix(variableId);
			},
			utility: {
				bg: ['clr.bg.*'],
				text: ['clr.text.*'],
				border: ['clr.border.*']
			}
		})
	]
});
