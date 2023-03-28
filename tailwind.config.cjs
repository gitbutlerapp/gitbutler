const config = {
	content: ['./src/**/*.{html,js,svelte,ts}'],
	darkMode: 'class',
	theme: {
		fontFamily: {
			sans: ['Inter', 'SF Pro', '-apple-system', 'system-ui'],
			mono: ['SF Mono', 'Consolas', 'Liberation Mono', 'monospace']
		},
		fontSize: {
			xs: '10px',
			sm: '12px',
			base: '13px',
			lg: '15px',
			xl: '22px',
			'2xl': '26px',
			'3xl': '30px'
		},
		extend: {
			colors: {
				gb: {
					700: '#52525B',
					750: '#3F3F46',
					800: '#3B3B3F',
					900: '#2F2F33'
				}
			}
		}
	},
	plugins: []
};

module.exports = config;
