const pxToRem = require('postcss-pxtorem');
const postcssPresetEnv = require('postcss-preset-env');

const config = {
	plugins: [
		pxToRem({
			rootValue: 16,
			unitPrecision: 5,
			propList: ['*'],
			selectorBlackList: [],
			replace: true,
			mediaQuery: false,
			minPixelValue: 0
		}),
    postcssPresetEnv()
	]
};

module.exports = config;
