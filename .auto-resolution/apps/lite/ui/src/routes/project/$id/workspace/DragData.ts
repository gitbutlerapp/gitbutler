import type { Operand } from "#ui/operands.ts";

export type DragData = {
	source: Operand;
};

export const parseDragData = (data: unknown): DragData | null => {
	if (typeof data !== "object" || data === null || !("source" in data)) return null;
	return data as DragData;
};
