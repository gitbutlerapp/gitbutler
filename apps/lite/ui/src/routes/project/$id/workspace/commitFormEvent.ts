/** Marks the expanded commit form, so a peer handler can tell that a press was made inside it. */
export const COMMIT_FORM_ATTRIBUTE = "data-commit-form";

/**
 * Whether a key press was made inside the expanded commit form.
 *
 * Escape is claimed by more than one handler at once, each registered with
 * `conflictBehavior: "allow"`. A press made in the form is the form's: the checked-file toolbox has
 * to leave the selection alone, or closing the form would mean re-checking everything the form was
 * opened to commit. Asked of the event's target rather than `document.activeElement`, because the
 * form's own handler moves focus out and either handler may run first.
 */
export const isCommitFormKeyEvent = (event: KeyboardEvent): boolean =>
	event.target instanceof Element && event.target.closest(`[${COMMIT_FORM_ATTRIBUTE}]`) !== null;
