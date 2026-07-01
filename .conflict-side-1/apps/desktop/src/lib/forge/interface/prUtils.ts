/**
 * Computes the status of a pull request based on its state.
 * Returns the status in order of priority: merged > closed > draft > open
 */
export function getPrStatus(pr: {
	mergedAt?: string;
	closedAt?: string;
	draft: boolean;
}): "merged" | "closed" | "draft" | "open" {
	if (pr.mergedAt) return "merged";
	if (pr.closedAt) return "closed";
	if (pr.draft) return "draft";
	return "open";
}
