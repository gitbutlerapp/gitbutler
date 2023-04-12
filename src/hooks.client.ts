import type { HandleClientError } from '@sveltejs/kit';

// This will catch errors in load functions from +page.ts files
export const handleError = (({ error, event }: { error: any; event: any }) => {
	console.error(error, event);
	return {
		message: error.message
	};
}) satisfies HandleClientError;
