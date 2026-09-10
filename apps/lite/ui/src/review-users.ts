import type { ForgeReviewUser } from "@gitbutler/but-sdk";

/**
 * Whether the user is an agent of any kind — Copilot, CI, a review bot.
 * The forge's own flag when it survives the trip, else the `[bot]` login
 * suffix and Copilot logins used by older cached GraphQL responses.
 */
export const isAgent = (user: Pick<ForgeReviewUser, "isBot" | "login">): boolean =>
	user.isBot || /(?:\[bot\]$|^copilot$|^copilot-pull-request-reviewer$)/i.test(user.login);

/**
 * Whether two logins name the same account. GitHub logins are
 * case-insensitive, and a bot's carries the `[bot]` suffix in REST but not
 * in GraphQL; both spellings reach the app.
 */
export const sameLogin = (a: string, b: string): boolean => loginKey(a) === loginKey(b);

/** The form of a login to key by, so both spellings of a bot land together. */
export const loginKey = (login: string): string => login.toLowerCase().replace(/\[bot\]$/, "");
