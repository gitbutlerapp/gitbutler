import type { SelectionSide } from "@pierre/diffs";

export type DiffLineTarget = {
	itemId: string;
	lineNumber: number;
	side: SelectionSide;
	/** Context lines belong to no one changed run of the hunk. */
	lineType: "change" | "context";
};

/** The attributes naming a line, on the gutter cell and on the code respectively. */
export const LINE_NUMBER_ATTRIBUTES = ["data-column-number", "data-line"];

/** What the row says about itself, before the line it names is read. */
type DiffLine = Pick<DiffLineTarget, "side" | "lineType">;

const diffLineFromElement = (element: HTMLElement): DiffLine | null => {
	switch (element.getAttribute("data-line-type")) {
		case "change-addition":
			return { side: "additions", lineType: "change" };
		case "change-deletion":
			return { side: "deletions", lineType: "change" };
		case "context":
			// Context has no side of its own, so it is numbered by the column holding it: the deletions
			// one in a split diff, the additions one otherwise.
			return {
				side: element.closest("[data-deletions]") ? "deletions" : "additions",
				lineType: "context",
			};
		default:
			return null;
	}
};

export const diffLineTargetFromElement = ({
	element,
	itemId,
}: {
	element: HTMLElement;
	itemId: string;
}): DiffLineTarget | null => {
	const line = diffLineFromElement(element);
	if (!line) return null;

	const [number] = LINE_NUMBER_ATTRIBUTES.flatMap(
		(attribute) => element.getAttribute(attribute) ?? [],
	);
	const lineNumber = Number.parseInt(number ?? "", 10);
	if (!Number.isFinite(lineNumber)) return null;

	return {
		itemId,
		lineNumber,
		...line,
	};
};
