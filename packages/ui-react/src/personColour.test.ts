import { describe, expect, test } from "vitest";
import { GLITCH_SIZE, glitchPattern, personColour } from "./personColour.ts";

const logins = [
	"krlvi",
	"schacon",
	"Caleb-T-Owens",
	"mtsgrd",
	"slarse",
	"samhh",
	"estib-vega",
	"PavelLaptev",
];

/**
 * Share of what the circle shows that the pattern covers: the cells whose centres fall
 * inside it, since the corners are cropped away.
 */
const coverage = (seed: string): number => {
	const half = GLITCH_SIZE / 2;
	const inCircle = (x: number, y: number) =>
		(x + 0.5 - half) ** 2 + (y + 0.5 - half) ** 2 < half ** 2;
	let shown = 0;
	let covered = 0;
	for (const m of glitchPattern(seed).matchAll(/M(\d+) (\d+)h(\d+)/g)) {
		const [x, y, run] = [Number(m[1]), Number(m[2]), Number(m[3])];
		for (let k = 0; k < run; k++) if (inCircle(x + k, y)) covered++;
	}
	for (let y = 0; y < GLITCH_SIZE; y++)
		for (let x = 0; x < GLITCH_SIZE; x++) if (inCircle(x, y)) shown++;

	return covered / shown;
};

describe("personColour", () => {
	test("gives the same colour for the same seed", () => {
		expect(personColour("krlvi")).toEqual(personColour("krlvi"));
	});

	test("shades the pattern in the ground's own hue", () => {
		for (const login of logins) {
			const { ground, shade } = personColour(login);
			expect(ground.replace("-80", "-70"), login).toBe(shade);
		}
	});
});

describe("glitchPattern", () => {
	test("gives the same pattern for the same seed", () => {
		expect(glitchPattern("krlvi")).toBe(glitchPattern("krlvi"));
	});

	test("gives different people different patterns", () => {
		expect(new Set(logins.map(glitchPattern)).size).toBe(logins.length);
	});

	test("keeps both tones inside the circle", () => {
		for (const login of logins) {
			expect(coverage(login), login).toBeGreaterThan(0.2);
			expect(coverage(login), login).toBeLessThan(0.8);
		}
	});
});
