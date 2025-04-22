/**
 * Polling for "real time" updates, like new messages recieved.
 * Should be used sparingly and only on light requests.
 */
export const POLLING_FAST = 30 * 1000;

/**
 * Polling for entities that are the current user focus and change requently.
 */
export const POLLING_REGULAR = 60 * 1000;

/**
 * Polling for lists that change less frequently or are more expensive to
 * query.
 */
export const POLLING_SLOW = 5 * 60 * 1000;

/**
 * Polling for expensive queries, or for things that change very infrequently.
 */
export const POLLING_GLACIALLY = 15 * 60 * 1000;
