import type { BehaviorSubject } from 'rxjs';

/**
 * Like the `BehaviorSubject`, but is missing the `next` method. This is like
 * what a `Readable` is to a `Writable`.
 *
 * You _could_ convert cast it back to a `Subject`, but then it would be
 * missing the `.value` property and `.getValue()` method.
 */
export type ReadonlyBehaviorSubject<T> = Omit<BehaviorSubject<T>, 'next'>;
