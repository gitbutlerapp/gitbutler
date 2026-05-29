import { dev } from "$app/environment";
import { applyIpcFingerprint } from "$lib/error/reduxError";
import * as Sentry from "@sentry/sveltekit";
import { PUBLIC_SENTRY_ENVIRONMENT } from "$env/static/public";

const { setUser, init, globalHandlersIntegration } = Sentry;

export function initSentry() {
	init({
		enabled: !dev && import.meta.env.VITE_E2E !== "true",
		dsn: "https://a35bbd6688a3a8f76e4956c6871f414a@o4504644069687296.ingest.sentry.io/4505976067129344",
		environment: PUBLIC_SENTRY_ENVIRONMENT,
		tracesSampleRate: 0,
		tracePropagationTargets: ["localhost", /gitbutler\.com/i],
		// `hooks.client.ts` installs a `window.onunhandledrejection` handler
		// that routes through `logError`, which wraps Tauri-shaped
		// `{name, message, code}` rejections into proper `Error` instances
		// before capture. Sentry's default `GlobalHandlers` integration adds
		// its own `addEventListener('unhandledrejection', ...)` that fires in
		// parallel and captures the raw object, producing a duplicate event
		// that all bucket into one generic "Object captured as promise
		// rejection" issue. Disable that half of the integration; keep
		// `onerror` as a safety net for sync errors that escape SvelteKit's
		// `handleError`.
		integrations: (defaults) => [
			...defaults.filter((i) => i.name !== "GlobalHandlers"),
			globalHandlersIntegration({ onerror: true, onunhandledrejection: false }),
		],
		// Override Sentry's stack-based grouping for IPC errors. The stack
		// inside `tauriInvoke` / `webInvoke` is uniform across calls into a
		// single backend command, which buckets distinct backend failures
		// (reflog write, worktree conflict, missing anchor) into one Sentry
		// issue. Use [command, normalised-first-line] instead, so each root
		// cause gets its own bucket and prod triage isn't fighting one
		// mega-issue per endpoint.
		beforeSend: applyIpcFingerprint,
	});
}

export function setSentryUser(user: { id: number; email?: string; name?: string }) {
	setUser({
		id: user.id.toString(),
		email: user.email,
		username: user.name,
	});
}

export function resetSentry() {
	setUser(null);
}
