import { queryOptions, skipToken, type QueryClient } from "@tanstack/react-query";
import * as idb from "idb-keyval";

export type ButDevSession = { login: string; token: string };
const sessionKey = "but-dev-session-v1";

export const butDevSessionQueryOptions = queryOptions({
	queryKey: ["butDev", "session"],
	queryFn: async () => (await idb.get<ButDevSession>(sessionKey)) ?? null,
});

type LoginStart = {
	device_code: string;
	verify_url: string;
	user_code: string;
	interval: number;
	expires_in: number;
};
type LoginFlow = LoginStart & { expiresAt: number };
type LoginResult =
	| { status: "pending" }
	| { status: "expired" }
	| { status: "complete"; node_token: string; login: string };

async function pollLogin(flow: LoginStart): Promise<LoginResult> {
	const response = await fetch("https://but.dev/api/cli/login/poll", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({
			device_code: flow.device_code,
			node: {
				name: "GitButler Lite",
				hostname: "GitButler Lite",
				os: navigator.platform,
				arch: "unknown",
			},
			viewer: true,
		}),
		credentials: "omit",
		redirect: "error",
		signal: AbortSignal.timeout(45_000),
	});
	if (!response.ok) throw new Error(`but.dev request failed (${response.status}).`);
	return response.json() as Promise<LoginResult>;
}

export async function startButDevLogin(): Promise<LoginFlow> {
	const response = await fetch("https://but.dev/api/cli/login", {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: "{}",
		credentials: "omit",
		redirect: "error",
		signal: AbortSignal.timeout(45_000),
	});
	if (!response.ok) throw new Error(`but.dev request failed (${response.status}).`);
	const flow = (await response.json()) as LoginStart;
	// Establish a viewer before the browser can approve this as a reporting machine.
	await pollLogin(flow);
	return { ...flow, expiresAt: Date.now() + flow.expires_in * 1000 };
}

export const butDevLoginQueryOptions = (flow: LoginFlow | undefined) =>
	queryOptions({
		// oxlint-disable-next-line @tanstack/query/exhaustive-deps -- The public code identifies this attempt; keep the private device code out of query keys.
		queryKey: ["butDev", "login", flow?.user_code],
		queryFn: flow
			? async ({ signal, client }) => {
					if (Date.now() >= flow.expiresAt)
						throw new Error("Sign-in expired. Connect again to get a new code.");

					// Let an in-flight poll finish: it may mint a token we must revoke on cancellation.
					const result = await pollLogin(flow);
					if (result.status === "pending") return false;
					if (result.status === "expired")
						throw new Error("Sign-in expired. Connect again to get a new code.");

					const session: ButDevSession = { login: result.login, token: result.node_token };
					try {
						signal.throwIfAborted();
						await idb.set(sessionKey, session);
						signal.throwIfAborted();
						client.setQueryData(butDevSessionQueryOptions.queryKey, session);
						return true;
					} catch (error) {
						await Promise.allSettled([
							idb.update<ButDevSession | null>(sessionKey, (current) =>
								current?.token === session.token ? null : (current ?? null),
							),
							fetch("https://but.dev/api/me/signout", {
								method: "POST",
								headers: {
									"Content-Type": "application/json",
									Authorization: `Bearer ${session.token}`,
								},
								body: "{}",
								credentials: "omit",
								redirect: "error",
								signal: AbortSignal.timeout(45_000),
							}),
						]);
						throw error;
					}
				}
			: skipToken,
		retry: false,
		gcTime: 0,
		enabled: (query) =>
			flow !== undefined && query.state.error === null && query.state.data !== true,
		refetchInterval: flow ? flow.interval * 1000 : false,
		refetchIntervalInBackground: true,
	});

export async function disconnectButDev(client: QueryClient): Promise<string | null> {
	const session = client.getQueryData(butDevSessionQueryOptions.queryKey);
	await idb.del(sessionKey);
	client.setQueryData(butDevSessionQueryOptions.queryKey, null);
	client.removeQueries({ queryKey: ["butDev", "mesh"] });
	if (!session) return null;
	try {
		const response = await fetch("https://but.dev/api/me/signout", {
			method: "POST",
			headers: {
				"Content-Type": "application/json",
				Authorization: `Bearer ${session.token}`,
			},
			body: "{}",
			credentials: "omit",
			redirect: "error",
			signal: AbortSignal.timeout(45_000),
		});
		if (!response.ok) throw new Error(`but.dev request failed (${response.status}).`);
		return null;
	} catch {
		return "Disconnected on this device, but but.dev could not revoke the token. It may still be valid on the server.";
	}
}
