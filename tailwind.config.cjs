const plugin = require('tailwindcss/plugin');
const config = {
	content: ['./src/**/*.{html,js,svelte,ts}'],
	darkMode: 'class',
	corePlugins: {
		backgroundOpacity: false
	},
	theme: {
		extend: {
			transitionProperty: {
				width: 'width'
			}
		},
		fontFamily: {
			sans: ['Inter', 'SF Pro', '-apple-system', 'system-ui'],
			mono: ['SF Mono', 'Consolas', 'Liberation Mono', 'monospace']
		},
		fontSize: {
			xs: '0.625rem',
			sm: '0.6875rem',
			base: '0.8125rem',
			lg: '0.9375rem',
			xl: '1.125rem',
			'2xl': '1.375rem',
			'3xl': '1.6875rem',
			'4xl': '2rem'
		},
		colors: {
			modal: {
				background: '#242429',
				stroke: '#3f3f3f'
			},
			gb: {
				700: '#52525B'
			},
			interactive: '#2563EB',
			divider: '#3f3f46',
			card: {
				active: '#3B3B3F',
				default: '#2F2F33'
			},
			app: {
				gradient: '#27272A'
			},
			overlay: {
				default: '#18181B'
			},
			text: {
				default: '#D4D4D8',
				subdued: '#71717A'
			},
			icon: {
				default: '#A1A1AA'
			},
			bookmark: {
				selected: '#2563EB'
			},
			white: '#FFFFFF',
			black: '#000000',
			transparent: 'transparent',
			gray: {
				400: '#9ca3af',
				500: '#6B7280'
			},
			dark: {
				50: '#FAFAFA', // Button text
				100: '#D4D4D8', // Normal text
				200: '#A1A1AA', // Commit sequence line, meatballs menu
				300: '#71717A', // Subtle text
				400: '#545454', // File card border
				500: '#414141', // Commit message border
				600: '#393939', // Commit card border
				700: '#373737', // Commit message background
				800: '#2C2C2C', // Tray and commit card background
				900: '#212121', // Header background
				1000: '#1E1E1E' // Board and inactive tab background
			},
			light: {
				50: '#F6F6F7', // Active tab and file card background
				100: '#F4F4F5', // Branch background in tray
				200: '#E4E4E7', // Board background and commit message border
				300: '#DDDDDE', // File card border
				400: '#D4D4D8', // Branch lane border and disabled text
				500: '#CDCDD0', // Scroll bar color
				600: '#A1A1AA', // Commit sequence line, meatballs menu, icons
				700: '#6D7175', // Subtle text
				800: '#3F3F46', // Branch text in tray
				900: '#202223' // Normal text
			},
			blue: {
				200: '#bfdbfe',
				400: '#60a5fa',
				500: '#3b82f6',
				600: '#2563eb',
				700: '#1d4ed8',
				900: '#1e3a8a'
			},
			yellow: {
				400: '#facc15',
				500: '#eab308',
				900: '#713f12'
			},
			red: {
				400: '#F87171',
				500: '#ef4444',
				600: '#dc2626',
				700: '#b91c1c',
				900: '#7c2d12'
			},
			green: {
				200: '#AFEDB1',
				300: '#6BE66D',
				400: '#4ade80',
				450: '#40C341',
				460: '#346E45',
				470: '#314D39',
				500: '#22c55e',
				600: '#16a34a',
				700: '#15803d',
				900: '#14532d'
			},
			purple: {
				600: '#58529F'
			},
			orange: {
				200: '#fed7aa'
			},
			zinc: {
				50: '#fafafa',
				100: '#f4f4f5',
				200: '#e5e5e5',
				300: '#d4d4d8',
				400: '#a1a1aa',
				500: '#71717a',
				600: '#52525b',
				700: '#3f3f46',
				800: '#27272a',
				900: '#18181b'
			}
		}
	},
	plugins: [
		// Expose color palette as CSS variables (--color-xxx-yyy)
		// https://gist.github.com/Merott/d2a19b32db07565e94f10d13d11a8574
		plugin(function ({ addBase, theme }) {
			function extractColorVars(colorObj, colorGroup = '') {
				return Object.keys(colorObj).reduce((vars, colorKey) => {
					const value = colorObj[colorKey];

					const newVars =
						typeof value === 'string'
							? { [`--color${colorGroup}-${colorKey}`]: value }
							: extractColorVars(value, `-${colorKey}`);

					return { ...vars, ...newVars };
				}, {});
			}

			addBase({
				':root': extractColorVars(theme('colors'))
			});
		})
	]
};

module.exports = config;
