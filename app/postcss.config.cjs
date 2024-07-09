const autoprefixer = require('autoprefixer');
const pxToRem = require('postcss-pxtorem');
const postcssNesting = require('postcss-nesting');

const config = {
	plugins: [
		//But others, like autoprefixer, need to run after,
		autoprefixer,
		pxToRem({
			rootValue: 16,
			unitPrecision: 5,
			propList: ['*'],
			selectorBlackList: [],
			replace: true,
			mediaQuery: false,
			minPixelValue: 0
		}),
		postcssNesting()
	]
};

module.exports = config;
