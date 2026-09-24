/**
 * The Gravatar photo for an email, and only a real one: an MD5 of the lowercased email, as
 * the backend keys a commit author's, with `d=404` so someone without a Gravatar gets no
 * generated face, just a failed load the caller falls back from.
 */
export const gravatarUrl = (email: string, size: number): string =>
	// Twice the drawn size, for a crisp image on a high-density screen.
	`https://www.gravatar.com/avatar/${md5(email.trim().toLowerCase())}?s=${size * 2}&r=g&d=404`;

/**
 * A picture URL with any Gravatar generated face turned off. The server and the backend
 * hand out Gravatar URLs that fall back to a face (`d=retro`, an initials image); this asks
 * for the real photo only, so someone without one fails to load and gets their colour
 * instead. Anything that isn't Gravatar passes through unchanged.
 */
export const withoutGeneratedFace = (url: string): string => {
	let parsed: URL;
	try {
		parsed = new URL(url);
	} catch {
		return url;
	}
	if (!parsed.hostname.endsWith("gravatar.com")) return url;
	parsed.searchParams.set("d", "404");
	parsed.searchParams.delete("default");
	parsed.searchParams.delete("f");
	parsed.searchParams.delete("forcedefault");
	return parsed.toString();
};

/**
 * MD5 over UTF-8, synchronously: Gravatar's hash, needed while rendering, and the browser
 * only hashes asynchronously. Not for anything secret.
 */
export const md5 = (input: string): string => {
	const bytes = new TextEncoder().encode(input);
	const words = new Uint32Array((((bytes.length + 8) >>> 6) + 1) * 16);
	const word = (i: number): number => words[i] ?? 0;
	bytes.forEach((byte, i) => {
		words[i >>> 2] = word(i >>> 2) | (byte << ((i % 4) * 8));
	});
	words[bytes.length >>> 2] = word(bytes.length >>> 2) | (0x80 << ((bytes.length % 4) * 8));
	words[words.length - 2] = (bytes.length * 8) >>> 0;
	words[words.length - 1] = Math.floor((bytes.length * 8) / 0x100000000);

	let a = 0x67452301;
	let b = 0xefcdab89;
	let c = 0x98badcfe;
	let d = 0x10325476;
	for (let block = 0; block < words.length; block += 16) {
		const [aa, bb, cc, dd] = [a, b, c, d];
		for (let i = 0; i < 64; i++) {
			let f: number;
			let g: number;
			if (i < 16) {
				f = (b & c) | (~b & d);
				g = i;
			} else if (i < 32) {
				f = (d & b) | (~d & c);
				g = (5 * i + 1) % 16;
			} else if (i < 48) {
				f = b ^ c ^ d;
				g = (3 * i + 5) % 16;
			} else {
				f = c ^ (b | ~d);
				g = (7 * i) % 16;
			}
			const shift = S[i] ?? 0;
			const sum = (a + f + (K[i] ?? 0) + word(block + g)) | 0;
			a = d;
			d = c;
			c = b;
			b = (b + ((sum << shift) | (sum >>> (32 - shift)))) | 0;
		}
		a = (a + aa) | 0;
		b = (b + bb) | 0;
		c = (c + cc) | 0;
		d = (d + dd) | 0;
	}
	return [a, b, c, d]
		.map((word) =>
			[0, 8, 16, 24]
				.map((shift) => ((word >>> shift) & 0xff).toString(16).padStart(2, "0"))
				.join(""),
		)
		.join("");
};

const S = [
	7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14,
	20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6,
	10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

const K = Array.from(
	{ length: 64 },
	(_, i) => Math.floor(Math.abs(Math.sin(i + 1)) * 0x100000000) | 0,
);
