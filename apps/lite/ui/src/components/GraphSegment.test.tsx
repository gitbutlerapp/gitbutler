/** @vitest-environment jsdom */

import { GraphSegment } from "./GraphSegment.tsx";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

const render = (groupCount?: number) => {
	const container = document.createElement("div");
	container.innerHTML = renderToStaticMarkup(
		<GraphSegment glyph="group" status="LocalOnly" groupCount={groupCount} />,
	);
	return container;
};

describe("folded commit groups", () => {
	it.each([
		[1, 1, "9.78571"],
		[2, 2, "6.28571"],
		[3, 3, "2.78571"],
		[8, 3, "2.78571"],
	] as const)("shows the rings for %i hidden commits", (count, rings, railEnd) => {
		const container = render(count);
		const svg = container.querySelector("svg");
		const paths = container.querySelectorAll("svg:first-child path");
		expect(paths[1]?.getAttribute("d")?.match(/M/g)).toHaveLength(rings);
		expect(paths[0]?.getAttribute("d")).toBe(`M8 0V${railEnd}M8 17.0038V26`);
		expect(svg?.getAttribute("viewBox")).toBe("0 0 16 26");
		expect(container.querySelectorAll("svg")).toHaveLength(2);
		expect(container.querySelector("svg:last-child path")?.getAttribute("d")).toBe("M8 0V28");
	});

	it("keeps three rings when no count is supplied", () => {
		expect(render().innerHTML).toBe(render(3).innerHTML);
	});

	it.each(["commit", "groupHead", "forkRight", "joinRight"] as const)(
		"leaves the %s glyph unchanged",
		(glyph) => {
			expect(
				renderToStaticMarkup(<GraphSegment glyph={glyph} status="LocalOnly" groupCount={1} />),
			).toBe(renderToStaticMarkup(<GraphSegment glyph={glyph} status="LocalOnly" />));
		},
	);
});
