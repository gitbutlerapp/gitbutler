import pluginCSS from '@cobalt-ui/plugin-css';

function pxToRem(token) {
	if (token.$type === 'dimension' && token.$value.slice(-2) === 'px') {
		return token.$value.slice(0, -2) / 16 + 'rem';
	}
}

export default {
	tokens: './design-tokens.json',
	outDir: './src/styles/core',
	plugins: [
		pluginCSS({
			filename: 'design-tokens.css',
			modeSelectors: [
				{
					mode: 'dark',
					selectors: [':root.dark']
				}
			],
			p3: false,
			colorFormat: 'hex',
			transform: pxToRem
		})
	]
};
