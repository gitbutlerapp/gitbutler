import DOMPurify from 'isomorphic-dompurify';
import { marked as markedLib } from 'marked';

/** DOMPurify cecommended by marked. */
export function marked(value: string) {
	return DOMPurify.sanitize(markedLib(value, { async: false }));
}
