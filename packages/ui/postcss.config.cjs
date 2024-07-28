const autoprefixer = require('autoprefixer');
const pxToRem = require('postcss-pxtorem');
const postcssNesting = require('postcss-nesting');
const postcssBundler = require('@csstools/postcss-bundler');
const postcssMinify = require('postcss-minify');

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
		autoprefixer(),
		postcssNesting(),
		postcssBundler(),
		postcssMinify()
	]
};

module.exports = config;
