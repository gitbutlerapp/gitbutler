import { getCloudApiClient } from './api/cloud/api';
import lscache from 'lscache';

const cloud = getCloudApiClient();

export async function summarizeHunk(diff: string): Promise<string> {
	const diffHash = hash(diff);

	if (lscache.get(diffHash)) {
		return lscache.get(diffHash);
	}

	const rsp = await cloud.summarize.hunk({ hunk: diff });
	lscache.set(diffHash, rsp.message, 1440); // 1 day ttl
	return rsp.message;
}

function hash(s: string) {
	let h = 0;
	let i = s.length;
	while (i > 0) {
		h = ((h << 5) - h + s.charCodeAt(--i)) | 0;
	}
	return h.toString();
}
