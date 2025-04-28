/**
 * Interface for handlers that can be passed to the `Dropzone` component.
 *
 * @example
 * const handler = {
 *   accepts(data: unkown) {
 *     return dropData instanceof CommitDropData;
 *   }
 *   ondrop(data: CommitDropData) {
 *     console.log("You dropped this: ", data) ;
 *   }
 * }
 */
export interface DropzoneHandler {
	accepts(data: unknown): boolean;
	ondrop(data: unknown): void;
}
