import type { Writable } from "svelte/store";

export interface SegmentContext {
	selectedSegmentId: Writable<string | undefined>;
	size: "default" | "small";
	registerSegment(id: string): void;
	selectSegment(id: string): void;
}
