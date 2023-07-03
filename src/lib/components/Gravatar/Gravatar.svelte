<script lang="ts">
	import * as cryptojs from 'crypto-js';

	export let size = 50;
	export let alt: string | undefined = undefined;
	export let rating = 'g';
	export let md55 = '';
	export let protocol = 'https://';
	export let email: string | undefined;
	export let domain = 'en.gravatar.com';

	let hasSrcset = typeof window === 'undefined' ? true : 'srcset' in document.createElement('img');

	$: formattedEmail = (email || '').trim().toLowerCase();
	$: def = $$props.default || 'retro';
	$: extraClass = $$props.class || '';
	$: extraStyles = $$props.style || '';
	$: base = `${protocol}${domain}/avatar/`;
	$: queryString = getQuery(size, rating, def, 1);
	$: retinaQuerystring = getQuery(size, rating, def, 2);
	$: hash = md55 || cryptojs.MD5(formattedEmail, { encoding: 'binary' });
	$: src = `${base}${hash}?${queryString}`;
	$: retinaSrc = `${base}${hash}?${retinaQuerystring}`;
	$: formattedSize = typeof size === 'number' ? `${size}px` : size;

	function getQuery(s: number, r: string, d: string, m: number) {
		return `s=${s * m}&r=${r}&d=${d}`;
	}

	function isRetina() {
		if (typeof window !== 'undefined' && !!window) {
			let mediaQuery =
				'(-webkit-min-device-pixel-ratio: 1.25), (min--moz-device-pixel-ratio: 1.25), (-o-min-device-pixel-ratio: 5/4), (min-resolution: 1.25dppx)';
			if (window.devicePixelRatio > 1.25) {
				return true;
			}
			if (window.matchMedia && window.matchMedia(mediaQuery).matches) {
				return true;
			}
		}
		return false;
	}
</script>

<img
	title={typeof alt === 'undefined' ? `Gravatar for ${formattedEmail}` : alt}
	alt={typeof alt === 'undefined' ? `Gravatar for ${formattedEmail}` : alt}
	style={extraStyles}
	class={extraClass}
	src={isRetina() && !hasSrcset ? retinaSrc : src}
	srcset={hasSrcset ? `${retinaSrc} 2x` : undefined}
	width={formattedSize}
	height={formattedSize}
	on:error
/>
