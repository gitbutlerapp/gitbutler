/**
 * Helper function to handle cookie management for attribution
 * Sets cookies for tracking referrer and first page visited if they don't exist and user is not authenticated
 */
export function handleAttributionCookies(event: {
	cookies: {
		get: (name: string) => string | undefined;
		set: (name: string, value: string, options: any) => void;
	};
	request: { headers: { get: (name: string) => string | null } };
	url: URL;
}) {
	// Check if user is authenticated via the auth token cookie
	const isAuthenticated = !!event.cookies.get('AuthService--token');

	const domain = new URL(event.url).hostname.split('.').slice(-2).join('.'); // This makes it accessible to subdomains

	// Set the standard cookie options
	const cookieOptions = {
		path: '/',
		domain: domain,
		httpOnly: true,
		secure: event.url.protocol === 'https:',
		sameSite: 'lax',
		maxAge: 60 * 60 * 24 * 30 // 30 days
	};

	// Handle referrer cookie
	const hasReferrerCookie = event.cookies.get('gb_referrer');
	if (!hasReferrerCookie && !isAuthenticated) {
		// Check for referrer in priority order
		const referrer =
			event.request.headers.get('referer') ||
			event.url.searchParams.get('utm_source') ||
			event.url.searchParams.get('ref');

		if (referrer) {
			// Set referrer cookie
			event.cookies.set('gb_referrer', referrer, cookieOptions);
		}
	}

	// Handle first page cookie
	const hasFirstPageCookie = event.cookies.get('gb_first_page');
	if (!hasFirstPageCookie && !isAuthenticated) {
		// Set first page cookie with current URL
		event.cookies.set('gb_first_page', event.url.href, cookieOptions);
	}
}
