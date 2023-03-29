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
		colors: {
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
			icon: {
				default: '#A1A1AA'
			},
			white: '#FFFFFF',
			transparent: 'transparent',
			gray: {
				400: '#9ca3af',
				500: '#6B7280'
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
				700: '#b91c1c',
				900: '#7c2d12'
			},
			green: {
				400: '#4ade80',
				600: '#16a34a',
				700: '#15803d',
				900: '#14532d'
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
	plugins: []
};

module.exports = config;
