import { goto } from "$app/navigation";
import type { WebRoutesService } from "@gitbutler/shared/routing/webRoutes.svelte";

// Kept in session storage, as signing in leaves this site and comes back
// through app.gitbutler.com, which drops anything carried in the URL.
const PAGE_BEFORE_LOGIN_KEY = "pageBeforeLogin";

/**
 * Go to the login page, saving the current page first, so signing in returns to it.
 */
export function goToLogin(routes: WebRoutesService) {
	savePageBeforeLogin();
	goto(routes.loginPath());
}

/**
 * Go to the signup page, saving the current page first, so signing up in this tab returns to
 * it. An email confirmation opens a tab of its own, which doesn't have it.
 */
export function goToSignup(routes: WebRoutesService) {
	savePageBeforeLogin();
	goto(routes.signupPath());
}

/**
 * Save the current page, so signing in returns to it.
 */
function savePageBeforeLogin() {
	sessionStorage.setItem(PAGE_BEFORE_LOGIN_KEY, window.location.href);
}

/**
 * The page saved before signing in, as a path on this site. It's forgotten
 * once read, so only the sign-in it was saved for returns to it.
 */
export function getPageBeforeLogin(): string | undefined {
	const stored = sessionStorage.getItem(PAGE_BEFORE_LOGIN_KEY);
	sessionStorage.removeItem(PAGE_BEFORE_LOGIN_KEY);
	if (!stored) return undefined;

	const url = new URL(stored, window.location.origin);
	if (url.origin !== window.location.origin) return undefined;
	return `${url.pathname}${url.search}${url.hash}`;
}
