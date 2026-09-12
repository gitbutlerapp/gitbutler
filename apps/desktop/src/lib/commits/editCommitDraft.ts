/**
 * In-progress reword draft for a single commit. Persisted in lane UI state so it
 * survives navigating away from the commit and back, instead of living only in
 * the editor component's local state. See issue #13287.
 */
export type EditCommitMessage = {
	commitId: string;
	title: string;
	description: string;
};

/**
 * Picks the title/description to seed the reword editor with: the persisted
 * draft when it belongs to `commitId`, otherwise the commit's own message.
 */
export function resolveEditDraft(
	draft: EditCommitMessage | undefined,
	commitId: string,
	original: { title: string; description: string },
): { title: string; description: string } {
	if (draft?.commitId === commitId) {
		return { title: draft.title, description: draft.description };
	}
	return original;
}

/**
 * Merges a partial editor change into the draft for `commitId`. The editor emits
 * one field at a time, so the untouched field is carried over from the existing
 * draft, or seeded from the original message when starting fresh.
 */
export function applyEditDraftUpdate(
	draft: EditCommitMessage | undefined,
	commitId: string,
	original: { title: string; description: string },
	update: { title?: string; description?: string },
): EditCommitMessage {
	const base = draft?.commitId === commitId ? draft : { commitId, ...original };
	return {
		commitId,
		title: update.title ?? base.title,
		description: update.description ?? base.description,
	};
}
