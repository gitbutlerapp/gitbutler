import {
	BrowserWindow,
	type HeadersReceivedResponse,
	type OnBeforeSendHeadersListenerDetails,
	type OnHeadersReceivedListenerDetails,
} from "electron";

const authPaths = new Set(["/api/cli/login", "/api/cli/login/poll", "/api/me/signout"]);
const allowedHeaders = new Set(["authorization", "content-type"]);

export function createButDevCors(isTrustedOrigin: (url: URL) => boolean) {
	const requests = new Map<number, { url: string; origin: string; method: string }>();

	function beforeSendHeaders(details: OnBeforeSendHeadersListenerDetails): void {
		requests.delete(details.id);
		const url = new URL(details.url);
		if (
			url.origin !== "https://but.dev" ||
			!authPaths.has(url.pathname) ||
			url.username !== "" ||
			url.password !== ""
		)
			return;

		const { frame, webContents } = details;
		// An Origin header alone is not proof of a trusted renderer, including on preflights.
		if (
			!frame ||
			!webContents ||
			!BrowserWindow.fromWebContents(webContents) ||
			frame !== webContents.mainFrame
		)
			return;

		if (!URL.canParse(frame.url)) return;
		const rendererUrl = new URL(frame.url);
		if (!isTrustedOrigin(rendererUrl)) return;

		const headers = new Map(
			Object.entries(details.requestHeaders).map(([name, value]) => [name.toLowerCase(), value]),
		);
		const origin = `${rendererUrl.protocol}//${rendererUrl.host}`;
		if (headers.get("origin") !== origin || headers.has("cookie")) return;

		if (details.method === "OPTIONS") {
			const method = headers.get("access-control-request-method");
			const requestedHeaders = headers.get("access-control-request-headers");
			if (
				method !== "POST" ||
				(requestedHeaders !== undefined &&
					!requestedHeaders
						.split(",")
						.every((name) => allowedHeaders.has(name.trim().toLowerCase())))
			)
				return;
		} else if (details.method !== "POST" || details.resourceType !== "xhr") {
			return;
		}

		requests.set(details.id, { url: details.url, origin, method: details.method });
	}

	function headersReceived(details: OnHeadersReceivedListenerDetails): HeadersReceivedResponse {
		const request = requests.get(details.id);
		requests.delete(details.id);
		if (!request || request.url !== details.url || request.method !== details.method) return {};

		const responseHeaders = Object.fromEntries(
			Object.entries(details.responseHeaders ?? {}).filter(
				([name]) => !name.toLowerCase().startsWith("access-control-"),
			),
		);
		responseHeaders["Access-Control-Allow-Origin"] = [request.origin];
		if (request.method !== "OPTIONS") return { responseHeaders };

		responseHeaders["Access-Control-Allow-Methods"] = ["POST"];
		responseHeaders["Access-Control-Allow-Headers"] = ["authorization, content-type"];
		responseHeaders["Access-Control-Max-Age"] = ["0"];
		return {
			responseHeaders,
			// but.dev does not implement OPTIONS. Only this validated preflight's 405
			// is synthesized as success; API responses and all other errors stay intact.
			...(details.statusCode === 405 ? { statusLine: "HTTP/1.1 200 OK" } : {}),
		};
	}

	return {
		beforeSendHeaders,
		headersReceived,
		forget: (id: number) => requests.delete(id),
	};
}
