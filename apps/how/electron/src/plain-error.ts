export function plainErrorMessage(error: unknown): string {
	const message = error instanceof Error ? error.message : String(error);

	if (message.includes("No git repository") || message.includes("not a git repository"))
		return "This folder is not ready for saving changes yet.";

	if (message.includes("Author") || message.includes("user.name") || message.includes("user.email"))
		return "How could not save because this project does not have author details configured.";

	if (message.includes("conflict") || message.includes("Conflict"))
		return "This project has work in a shape How cannot save automatically yet.";

	if (message.includes("Signing") || message.includes("signing"))
		return "How could not save because this project requires a signing step it cannot complete yet.";

	return "How could not save this checkpoint.";
}
