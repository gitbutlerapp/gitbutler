import { backend, listenNow } from "./backend.ts";

const urlOrNull = (link: string): URL | null => {
	try {
		return new URL(link);
	} catch {
		return null;
	}
};

const completeLogin = (link: string): void => {
	const url = urlOrNull(link);
	if (url?.host !== "login") return;
	const token = url.searchParams.get("access_token");
	if (token === null) return;
	backend("login_and_persist", { token }).catch(async (error: unknown) => {
		const { reportError } = await import("#ui/error-reporting.ts");
		reportError(error, { message: "Failed to sign in from a login link" });
	});
};

const completeLogins = (links: unknown): void =>
	(links as Array<string> | null)?.forEach(completeLogin);

void backend("plugin:deep-link|get_current").then(completeLogins);
listenNow("deep-link://new-url", completeLogins);
