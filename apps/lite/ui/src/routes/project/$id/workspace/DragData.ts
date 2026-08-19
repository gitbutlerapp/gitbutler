import type { Address } from "#ui/addresses.ts";

export type DragData = {
	sources: Array<Address>;
};

export const parseDragData = (data: unknown): DragData | null => {
	if (typeof data !== "object" || data === null || !("sources" in data)) return null;
	return data as DragData;
};
