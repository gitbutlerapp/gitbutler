import type { RelativeTo } from "@gitbutler/but-sdk";

export const relativeToKey = (relativeTo: RelativeTo): string => {
	switch (relativeTo.type) {
		case "reference":
			return JSON.stringify(["reference", relativeTo.subject]);
		case "referenceBytes":
			return JSON.stringify(["referenceBytes", relativeTo.subject]);
		case "commit":
			return JSON.stringify(["commit", relativeTo.subject]);
	}
};

export const relativeToEquals = (a: RelativeTo, b: RelativeTo): boolean =>
	relativeToKey(a) === relativeToKey(b);
